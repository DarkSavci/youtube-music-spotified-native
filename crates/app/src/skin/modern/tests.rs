use super::*;
use crate::skin::zip;

/// A picture of one colour, as a PNG; `alpha` for how solid it is.
fn png(width: u32, height: u32, color: [u8; 3], alpha: u8) -> Vec<u8> {
    let [red, green, blue] = color;
    let pixel = image::Rgba([red, green, blue, alpha]);
    let image = image::RgbaImage::from_pixel(width, height, pixel);
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("a picture");
    bytes.into_inner()
}

/// A small skin: a background, a group with a button and a slider in it,
/// a line of text, and an analyser.
pub(crate) fn skin() -> Vec<u8> {
    let root = br#"<WinampAbstractionLayer version="1.0">
        <include file="xml/player.xml"/>
    </WinampAbstractionLayer>"#;
    let player = br#"<elements>
          <bitmap id="bg" file="../pic/bg.png"/>
          <bitmap id="play" file="pic/buttons.png" x="0" y="0" w="20" h="10"/>
          <bitmap id="play.down" file="pic/buttons.png" x="0" y="10" w="20" h="10"/>
          <bitmap id="thumb" file="pic/buttons.png" x="20" y="0" w="8" h="10"/>
          <bitmapfont id="small" file="pic/font.png" charwidth="5" charheight="6" hspacing="1"/>
          <color id="ink" value="10,20,30"/>
        </elements>
        <groupdef id="controls" w="100" h="20">
          <Button id="Play" action="PLAY" x="5" y="5" image="play" downImage="play.down" tooltip="Play"/>
          <button id="Drawer" action="TOGGLE" param="guid:drawer" x="30" y="5" image="play"/>
          <togglebutton id="Shuffle" x="60" y="5" image="play" cfgattrib="{45F3F7C1};Shuffle"/>
          <slider action="VOLUME" x="-40" relatx="1" y="15" w="30" h="10" thumb="thumb"/>
          <layer id="hidden" image="play" visible="0"/>
        </groupdef>
        <container id="main" name="Main Window">
          <layout id="shade" background="play"/>
          <layout id="normal" background="bg">
            <group id="controls" x="10" y="40"/>
            <text display="songname" x="10" y="10" w="-20" relatw="1" h="6" font="small" color="ink"/>
            <text display="time" x="10" y="20" w="60" h="12" fontsize="11" color="1,2,3" align="right"/>
            <vis x="150" y="20" w="40" h="16" colorallbands="0,200,0"/>
            <script file="scripts/main.maki"/>
          </layout>
        </container>"#;
    zip::write(&[
        ("Skin/skin.xml", root, true),
        ("Skin/xml/player.xml", player, true),
        ("Skin/pic/bg.png", &png(200, 80, [9, 9, 9], 255), true),
        ("Skin/pic/buttons.png", &png(40, 20, [200, 0, 0], 255), true),
        ("Skin/pic/font.png", &png(155, 18, [0, 0, 0], 255), true),
    ])
}

#[test]
fn the_main_windows_normal_layout_is_laid_out_where_the_xml_puts_it() {
    let skin = Modern::from_archive("small", &skin()).expect("a skin");
    assert_eq!((skin.width, skin.height), (200, 80));
    // Solid all over: a rectangle, and no shape to cut.
    assert_eq!(skin.shape, None);
    let at = |item: &Item| (item.x, item.y, item.width, item.height);
    let [background, play, drawer, shuffle, volume, song, time, vis] = &skin.items[..] else {
        panic!("{} items: {:#?}", skin.items.len(), skin.items);
    };
    assert_eq!(at(background), (0, 0, 200, 80));
    // Inside the group, which is at 10, 40.
    assert_eq!(at(play), (15, 45, 20, 10));
    assert!(matches!(
        &play.kind,
        Kind::Button { act: Act::Play, down: Some(down), hint, .. }
            if down.y == 10 && hint == "Play"
    ));
    // What only a script could do is drawn, and does nothing.
    assert!(matches!(drawer.kind, Kind::Layer(_)));
    assert!(matches!(
        shuffle.kind,
        Kind::Button {
            act: Act::Shuffle,
            ..
        }
    ));
    // Forty from the group's right edge, which is a hundred wide.
    assert_eq!(at(volume), (70, 55, 30, 10));
    assert!(matches!(
        volume.kind,
        Kind::Slider {
            slide: Slide::Volume,
            vertical: false,
            ..
        }
    ));
    // Twenty narrower than the window.
    assert_eq!(at(song), (10, 10, 180, 6));
    assert!(matches!(
        &song.kind,
        Kind::Text { words: Words::Song, font: Some(font), color: [10, 20, 30], .. }
            if font.char_width == 5 && font.spacing == 1
    ));
    assert!(matches!(
        &time.kind,
        Kind::Text {
            words: Words::Time,
            font: None,
            color: [1, 2, 3],
            align: 1,
            ..
        }
    ));
    assert_eq!(at(vis), (150, 20, 40, 16));
}

#[test]
fn a_window_that_is_not_solid_all_over_has_the_shape_of_what_is() {
    // The background is see-through; a solid block half its width lies
    // over the left of it.
    let player = br#"<elements>
          <bitmap id="bg" file="bg.png"/>
          <bitmap id="block" file="block.png"/>
        </elements>
        <container id="main"><layout id="normal" background="bg">
          <layer image="block" x="0" y="0"/>
        </layout></container>"#;
    let archive = zip::write(&[
        ("skin.xml", player, false),
        ("bg.png", &png(40, 20, [0, 0, 0], 0), false),
        ("block.png", &png(20, 20, [5, 5, 5], 255), false),
    ]);
    let skin = Modern::from_archive("shaped", &archive).expect("a skin");
    let shape = skin.shape.expect("a shape");
    assert_eq!(shape.len(), 20);
    assert_eq!(shape[0], [0, 0, 20, 1]);
    assert_eq!(shape[19], [0, 19, 20, 20]);
}

#[test]
fn what_is_not_a_modern_skin_is_refused() {
    assert!(matches!(
        Modern::from_archive("text", b"just some text"),
        Err(SkinError::NotAnArchive)
    ));
    let classic = zip::write(&[("main.bmp", &png(275, 116, [1, 1, 1], 255), false)]);
    assert!(matches!(
        Modern::from_archive("classic", &classic),
        Err(SkinError::Empty)
    ));
    // A skin with nothing in its window to draw.
    let bare = zip::write(&[(
        "skin.xml",
        br#"<container id="main"><layout id="normal" w="300" h="100"/></container>"#,
        false,
    )]);
    assert!(matches!(
        Modern::from_archive("bare", &bare),
        Err(SkinError::Empty)
    ));
}

/// Reads every `.wal` in `$SPOTIFIED_MODERN_SAMPLES`, when set, to check
/// the reader against real skins without shipping any.
#[test]
fn sample_skins_load() {
    let Ok(dir) = std::env::var("SPOTIFIED_MODERN_SAMPLES") else {
        return;
    };
    for entry in std::fs::read_dir(dir).expect("the folder").flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "wal") {
            continue;
        }
        let bytes = std::fs::read(&path).expect("the file");
        match Modern::from_archive("sample", &bytes) {
            Ok(skin) => {
                let count = |wanted: fn(&Kind) -> bool| {
                    skin.items.iter().filter(|item| wanted(&item.kind)).count()
                };
                eprintln!(
                    "{}: {}x{}, {} sheets, {} layers, {} buttons, {} sliders, {} texts, shaped: {}",
                    path.display(),
                    skin.width,
                    skin.height,
                    skin.sheets.len(),
                    count(|kind| matches!(kind, Kind::Layer(_))),
                    count(|kind| matches!(kind, Kind::Button { .. })),
                    count(|kind| matches!(kind, Kind::Slider { .. })),
                    count(|kind| matches!(kind, Kind::Text { .. })),
                    skin.shape.is_some(),
                );
            }
            Err(error) => eprintln!("{}: {error}", path.display()),
        }
    }
}
