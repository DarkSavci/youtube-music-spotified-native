//! The system's own dialog for choosing files.

use std::path::PathBuf;

/// Asks for Winamp skin files and waits for the answer: the files chosen,
/// or none if the dialog was put away. Called on a thread of its own, so
/// the window goes on drawing while the dialog is up.
pub fn skins() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        win::skins()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
mod win {
    use std::path::PathBuf;

    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
        CoTaskMemFree, CoUninitialize,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FOS_ALLOWMULTISELECT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FileOpenDialog,
        IFileOpenDialog, SIGDN_FILESYSPATH,
    };
    use windows::core::w;

    pub(super) fn skins() -> Vec<PathBuf> {
        // SAFETY: COM is started on this thread for as long as the dialog
        // is used and stopped again only if this call started it.
        let started = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
        // Putting the dialog away is reported as a failure, and is none.
        let files = pick().unwrap_or_else(|error| {
            log::debug!("no skin was chosen: {error}");
            Vec::new()
        });
        if started {
            // SAFETY: pairs with the start above; nothing of COM's is held.
            unsafe { CoUninitialize() };
        }
        files
    }

    fn pick() -> windows::core::Result<Vec<PathBuf>> {
        let kinds = [COMDLG_FILTERSPEC {
            pszName: w!("Winamp skins"),
            pszSpec: w!("*.wsz;*.wal;*.zip"),
        }];
        // SAFETY: plain calls into COM on a thread where it is started.
        // The strings are static, and each name the dialog hands back is
        // copied before it is freed with the allocator that made it.
        unsafe {
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetFileTypes(&kinds)?;
            dialog.SetTitle(w!("Add a Winamp skin"))?;
            let options = dialog.GetOptions()?;
            let wanted = FOS_ALLOWMULTISELECT | FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM;
            dialog.SetOptions(options | wanted)?;
            dialog.Show(None)?;
            let chosen = dialog.GetResults()?;
            let mut files = Vec::new();
            for index in 0..chosen.GetCount()? {
                let name = chosen.GetItemAt(index)?.GetDisplayName(SIGDN_FILESYSPATH)?;
                files.push(PathBuf::from(String::from_utf16_lossy(name.as_wide())));
                CoTaskMemFree(Some(name.0.cast()));
            }
            Ok(files)
        }
    }
}
