//! Turning H.264 into pictures, with what Windows brings: its H.264
//! decoder, and its video processor to make of each picture the colours
//! and the size the screen wants. Both run on the graphics card where it
//! will have them, and in software where it will not.
//!
//! Everything Win32 about video is in this file. What leaves it is a time
//! and a row of pixels.

use std::mem::ManuallyDrop;

use super::Color32;
use windows::Win32::Foundation::{HMODULE, RECT};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_VIDEO_SUPPORT, D3D11_SDK_VERSION,
    D3D11CreateDevice, ID3D11Multithread,
};
use windows::Win32::Media::MediaFoundation::{
    CLSID_MSH264DecoderMFT, CLSID_VideoProcessorMFT, IMF2DBuffer, IMFDXGIDeviceManager,
    IMFMediaType, IMFSample, IMFTransform, IMFVideoProcessorControl, MF_E_NOTACCEPTING,
    MF_E_TRANSFORM_NEED_MORE_INPUT, MF_E_TRANSFORM_STREAM_CHANGE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_SA_D3D11_AWARE, MF_VERSION,
    MFCreate2DMediaBuffer, MFCreateDXGIDeviceManager, MFCreateMediaType, MFCreateMemoryBuffer,
    MFCreateSample, MFMediaType_Video, MFSTARTUP_LITE, MFShutdown, MFStartup,
    MFT_MESSAGE_COMMAND_FLUSH, MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,
    MFT_MESSAGE_NOTIFY_START_OF_STREAM, MFT_MESSAGE_SET_D3D_MANAGER, MFT_OUTPUT_DATA_BUFFER,
    MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES, MFT_OUTPUT_STREAM_PROVIDES_SAMPLES, MFVideoFormat_H264,
    MFVideoFormat_NV12, MFVideoFormat_RGB32, MFVideoInterlace_Progressive,
};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::core::{GUID, Interface};

/// Media Foundation, for as long as the thread that started it needs it.
pub struct Runtime {
    com: bool,
}

impl Runtime {
    pub fn start() -> Result<Self, String> {
        // SAFETY: both calls are paired with their closing ones in `drop`,
        // on the same thread, which is the only one that holds a Runtime.
        unsafe {
            let com = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
            if let Err(error) = MFStartup(MF_VERSION, MFSTARTUP_LITE) {
                if com {
                    CoUninitialize();
                }
                return Err(said("Media Foundation did not start", &error));
            }
            Ok(Self { com })
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // SAFETY: closes what `start` opened, on the thread that opened it,
        // after every decoder made on it has been dropped.
        unsafe {
            let _ = MFShutdown();
            if self.com {
                CoUninitialize();
            }
        }
    }
}

fn said(what: &str, error: &windows::core::Error) -> String {
    format!("{what}: {} ({:#x})", error.message(), error.code().0)
}

/// A decoded picture, not yet in the screen's colours. Making those costs
/// more than decoding, so a picture that is already late is dropped as this.
pub struct Decoded {
    sample: IMFSample,
    /// When it shows, in 100 ns.
    pub time: i64,
    pub duration: i64,
}

pub struct Decoder {
    decoder: IMFTransform,
    converter: IMFTransform,
    /// The graphics card's device, shared by both; `None` in software.
    manager: Option<IMFDXGIDeviceManager>,
    /// The picture as the track gives it, and as it leaves here.
    source: (u32, u32),
    size: (u32, u32),
    /// The converter has been told what the decoder now makes.
    converting: bool,
    /// Where the converter paints, kept from one picture to the next.
    canvas: Option<IMFSample>,
}

impl Decoder {
    /// A decoder for pictures of `source` size that hands them on at `size`.
    /// With `hardware`, on the graphics card or not at all.
    pub fn new(source: (u32, u32), size: (u32, u32), hardware: bool) -> Result<Self, String> {
        // SAFETY: plain COM calls on objects this function owns; every
        // pointer handed over is to a live local.
        unsafe {
            let manager = if hardware {
                Some(device_manager()?)
            } else {
                None
            };
            let decoder: IMFTransform =
                CoCreateInstance(&CLSID_MSH264DecoderMFT, None, CLSCTX_INPROC_SERVER)
                    .map_err(|error| said("no H.264 decoder", &error))?;
            if let Some(manager) = &manager {
                let aware = decoder
                    .GetAttributes()
                    .and_then(|attributes| attributes.GetUINT32(&MF_SA_D3D11_AWARE))
                    .unwrap_or(0);
                if aware == 0 {
                    return Err("the H.264 decoder does not use the graphics card".to_owned());
                }
                decoder
                    .ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, manager.as_raw() as usize)
                    .map_err(|error| said("the decoder refused the graphics card", &error))?;
            }
            let input = video_type(&MFVideoFormat_H264, source)?;
            decoder
                .SetInputType(0, &input, 0)
                .map_err(|error| said("the decoder refused the stream", &error))?;
            choose_output(&decoder)?;
            let converter: IMFTransform =
                CoCreateInstance(&CLSID_VideoProcessorMFT, None, CLSCTX_INPROC_SERVER)
                    .map_err(|error| said("no video processor", &error))?;
            if let Some(manager) = &manager {
                converter
                    .ProcessMessage(MFT_MESSAGE_SET_D3D_MANAGER, manager.as_raw() as usize)
                    .map_err(|error| said("the processor refused the graphics card", &error))?;
            }
            for message in [
                MFT_MESSAGE_NOTIFY_BEGIN_STREAMING,
                MFT_MESSAGE_NOTIFY_START_OF_STREAM,
            ] {
                let _ = decoder.ProcessMessage(message, 0);
            }
            Ok(Self {
                decoder,
                converter,
                manager,
                source,
                size,
                converting: false,
                canvas: None,
            })
        }
    }

    pub fn hardware(&self) -> bool {
        self.manager.is_some()
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Hands the decoder one picture's bytes. `Ok(false)` when it will take
    /// no more until what it has made is pulled.
    pub fn push(&mut self, data: &[u8], time: i64, duration: i64) -> Result<bool, String> {
        // SAFETY: the buffer is made here with room for `data`, and is
        // locked only for the copy into it.
        unsafe {
            let sample = memory_sample(data.len())?;
            let buffer = sample
                .GetBufferByIndex(0)
                .map_err(|error| said("no buffer", &error))?;
            let mut into = std::ptr::null_mut();
            buffer
                .Lock(&mut into, None, None)
                .map_err(|error| said("the buffer would not lock", &error))?;
            std::ptr::copy_nonoverlapping(data.as_ptr(), into, data.len());
            let _ = buffer.Unlock();
            let _ = buffer.SetCurrentLength(data.len() as u32);
            let _ = sample.SetSampleTime(time);
            let _ = sample.SetSampleDuration(duration);
            match self.decoder.ProcessInput(0, &sample, 0) {
                Ok(()) => Ok(true),
                Err(error) if error.code() == MF_E_NOTACCEPTING => Ok(false),
                Err(error) => Err(said("the decoder refused a picture", &error)),
            }
        }
    }

    /// The next picture the decoder has ready, or `None` when it wants more
    /// of the stream first.
    pub fn pull(&mut self) -> Result<Option<Decoded>, String> {
        // A change of format is answered by choosing the output again and
        // asking once more; twice in a row would be a decoder gone wrong.
        for _ in 0..3 {
            // SAFETY: see `output`.
            match unsafe { output(&self.decoder, None) } {
                Ok(Some(sample)) => {
                    // SAFETY: plain calls on the sample just received.
                    let (time, duration) = unsafe {
                        (
                            sample.GetSampleTime().unwrap_or(0),
                            sample.GetSampleDuration().unwrap_or(0),
                        )
                    };
                    return Ok(Some(Decoded {
                        sample,
                        time,
                        duration,
                    }));
                }
                Ok(None) => return Ok(None),
                Err(error) if error.code() == MF_E_TRANSFORM_STREAM_CHANGE => {
                    // SAFETY: plain calls on the decoder this owns.
                    unsafe { choose_output(&self.decoder)? };
                    self.converting = false;
                }
                Err(error) => return Err(said("decoding failed", &error)),
            }
        }
        Err("the decoder kept changing its mind about the format".to_owned())
    }

    /// Makes of a decoded picture the pixels egui draws, at this decoder's
    /// size. `pixels` is reused from picture to picture.
    pub fn convert(&mut self, decoded: Decoded, pixels: &mut Vec<Color32>) -> Result<(), String> {
        // SAFETY: COM calls on objects this owns; the locked rows are read
        // within the lock, and no further than the pitch and height allow.
        unsafe {
            if !self.converting {
                self.prepare_converter()?;
            }
            self.converter
                .ProcessInput(0, &decoded.sample, 0)
                .map_err(|error| said("the processor refused a picture", &error))?;
            let (width, height) = (self.size.0 as usize, self.size.1 as usize);
            let own = match self.canvas.take() {
                Some(own) => own,
                None => canvas(self.size)?,
            };
            let made = output(&self.converter, Some(own.clone()))
                .map_err(|error| said("the picture was not converted", &error))?
                .ok_or("the processor made no picture")?;
            let buffer = made
                .GetBufferByIndex(0)
                .map_err(|error| said("the picture has no buffer", &error))?;
            pixels.clear();
            pixels.reserve(width * height);
            let mut copy = |first: *const u8, pitch: isize| {
                for row in 0..height as isize {
                    let line = std::slice::from_raw_parts(first.offset(row * pitch), width * 4);
                    let (texels, _) = line.as_chunks::<4>();
                    pixels.extend(
                        texels
                            .iter()
                            .map(|bgr| Color32::from_rgb(bgr[2], bgr[1], bgr[0])),
                    );
                }
            };
            if let Ok(rows) = buffer.cast::<IMF2DBuffer>() {
                let mut first = std::ptr::null_mut();
                let mut pitch = 0i32;
                rows.Lock2D(&mut first, &mut pitch)
                    .map_err(|error| said("the picture would not lock", &error))?;
                copy(first, pitch as isize);
                let _ = rows.Unlock2D();
            } else {
                let mut first = std::ptr::null_mut();
                let mut length = 0u32;
                buffer
                    .Lock(&mut first, None, Some(&mut length))
                    .map_err(|error| said("the picture would not lock", &error))?;
                if (length as usize) < width * height * 4 {
                    let _ = buffer.Unlock();
                    return Err("the picture is smaller than it said".to_owned());
                }
                // RGB in plain memory is kept last row first.
                let pitch = (width * 4) as isize;
                copy(first.offset(pitch * (height as isize - 1)), -pitch);
                let _ = buffer.Unlock();
            }
            // The processor has let go of the canvas by now; the next
            // picture is painted on the same one.
            self.canvas = Some(own);
            Ok(())
        }
    }

    /// Has pictures leave at another size from here on.
    pub fn resize(&mut self, size: (u32, u32)) {
        if size == self.size {
            return;
        }
        self.size = size;
        self.converting = false;
        self.canvas = None;
        // SAFETY: a plain call on an object this owns.
        unsafe {
            let _ = self.converter.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
        }
    }

    /// Forgets everything in hand, for a jump to another place.
    pub fn flush(&mut self) {
        // SAFETY: plain calls on objects this owns.
        unsafe {
            let _ = self.decoder.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
            let _ = self.converter.ProcessMessage(MFT_MESSAGE_COMMAND_FLUSH, 0);
        }
    }

    /// Tells the converter what the decoder makes and what to make of it.
    unsafe fn prepare_converter(&mut self) -> Result<(), String> {
        unsafe {
            let made = self
                .decoder
                .GetOutputCurrentType(0)
                .map_err(|error| said("the decoder has no format", &error))?;
            self.converter
                .SetInputType(0, &made, 0)
                .map_err(|error| said("the processor refused the decoder's format", &error))?;
            let wanted = video_type(&MFVideoFormat_RGB32, self.size)?;
            self.converter
                .SetOutputType(0, &wanted, 0)
                .map_err(|error| said("the processor will not make RGB", &error))?;
            // A decoder works in blocks of sixteen, so its picture can be a
            // few rows taller than the film; only the film is wanted.
            if let Ok(control) = self.converter.cast::<IMFVideoProcessorControl>() {
                let film = RECT {
                    left: 0,
                    top: 0,
                    right: self.source.0 as i32,
                    bottom: self.source.1 as i32,
                };
                let _ = control.SetSourceRectangle(Some(&film));
            }
            self.converting = true;
            Ok(())
        }
    }
}

/// One output of a transform: a sample, or `None` when it needs more input.
/// `own` is the sample to fill for a transform that brings none of its own.
unsafe fn output(
    transform: &IMFTransform,
    own: Option<IMFSample>,
) -> windows::core::Result<Option<IMFSample>> {
    unsafe {
        let info = transform.GetOutputStreamInfo(0)?;
        let provides =
            (MFT_OUTPUT_STREAM_PROVIDES_SAMPLES.0 | MFT_OUTPUT_STREAM_CAN_PROVIDE_SAMPLES.0) as u32;
        let given = if info.dwFlags & provides != 0 {
            None
        } else {
            match own {
                Some(own) => Some(own),
                None => Some(memory_sample(info.cbSize as usize).map_err(|_| {
                    windows::core::Error::from(windows::Win32::Foundation::E_OUTOFMEMORY)
                })?),
            }
        };
        let mut buffers = [MFT_OUTPUT_DATA_BUFFER {
            dwStreamID: 0,
            pSample: ManuallyDrop::new(given),
            dwStatus: 0,
            pEvents: ManuallyDrop::new(None),
        }];
        let mut status = 0;
        let result = transform.ProcessOutput(0, &mut buffers, &mut status);
        // Both are taken back whatever happened, or they would never be
        // released.
        let sample = ManuallyDrop::take(&mut buffers[0].pSample);
        drop(ManuallyDrop::take(&mut buffers[0].pEvents));
        match result {
            Ok(()) => Ok(sample),
            Err(error) if error.code() == MF_E_TRANSFORM_NEED_MORE_INPUT => Ok(None),
            Err(error) => Err(error),
        }
    }
}

/// The graphics card's device in the wrapping Media Foundation shares it by.
unsafe fn device_manager() -> Result<IMFDXGIDeviceManager, String> {
    unsafe {
        let mut device = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_VIDEO_SUPPORT | D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            None,
        )
        .map_err(|error| said("no graphics device for video", &error))?;
        let device = device.ok_or("no graphics device for video")?;
        // The decoder works on threads of its own.
        if let Ok(threads) = device.cast::<ID3D11Multithread>() {
            let _ = threads.SetMultithreadProtected(true);
        }
        let mut token = 0;
        let mut manager = None;
        MFCreateDXGIDeviceManager(&mut token, &mut manager)
            .map_err(|error| said("no device manager", &error))?;
        let manager = manager.ok_or("no device manager")?;
        manager
            .ResetDevice(&device, token)
            .map_err(|error| said("the device manager refused the device", &error))?;
        Ok(manager)
    }
}

unsafe fn video_type(format: &GUID, size: (u32, u32)) -> Result<IMFMediaType, String> {
    unsafe {
        let made = || -> windows::core::Result<IMFMediaType> {
            let kind = MFCreateMediaType()?;
            kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            kind.SetGUID(&MF_MT_SUBTYPE, format)?;
            kind.SetUINT64(
                &MF_MT_FRAME_SIZE,
                (u64::from(size.0) << 32) | u64::from(size.1),
            )?;
            kind.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            Ok(kind)
        };
        made().map_err(|error| said("no media type", &error))
    }
}

/// Has the decoder make NV12, the form the processor and the card both take.
unsafe fn choose_output(decoder: &IMFTransform) -> Result<(), String> {
    unsafe {
        for index in 0.. {
            let Ok(offered) = decoder.GetOutputAvailableType(0, index) else {
                break;
            };
            if offered.GetGUID(&MF_MT_SUBTYPE).ok() == Some(MFVideoFormat_NV12) {
                return decoder
                    .SetOutputType(0, &offered, 0)
                    .map_err(|error| said("the decoder will not make NV12", &error));
            }
        }
        Err("the decoder offers no NV12".to_owned())
    }
}

/// A sample to paint RGB into, whose rows can be asked for top first.
unsafe fn canvas(size: (u32, u32)) -> Result<IMFSample, String> {
    /// `D3DFMT_X8R8G8B8`, which is what Media Foundation calls RGB32.
    const RGB32: u32 = 22;
    unsafe {
        let made = || -> windows::core::Result<IMFSample> {
            let sample = MFCreateSample()?;
            let buffer = MFCreate2DMediaBuffer(size.0, size.1, RGB32, false)?;
            sample.AddBuffer(&buffer)?;
            Ok(sample)
        };
        made().map_err(|error| said("no memory for a picture", &error))
    }
}

unsafe fn memory_sample(length: usize) -> Result<IMFSample, String> {
    unsafe {
        let made = || -> windows::core::Result<IMFSample> {
            let sample = MFCreateSample()?;
            let buffer = MFCreateMemoryBuffer(length as u32)?;
            sample.AddBuffer(&buffer)?;
            Ok(sample)
        };
        made().map_err(|error| said("no memory for a picture", &error))
    }
}
