//! The graphics and sounds of the 2005 freeware game *Dostihy 3000 Deluxe*
//! (see `LICENSE-original.txt`), read straight from its installer. They are not part of
//! this repository: the installer is an ordinary asset in the game's assets
//! folder (`crate::asset_root`) – on desktop downloaded there first when it's
//! missing – and is unpacked in memory into labeled assets, such as
//! `Dostihy3000setup.exe#horse/napoli`.

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext, RenderAssetUsages};
use bevy::audio::AudioSource;
use bevy::prelude::*;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::collections::HashMap;
use std::error::Error;

pub const INSTALLER: &str = "Dostihy3000setup.exe";
/// BLAKE3 of the installer this was written for.
const INSTALLER_BLAKE3: &str = "3f39ba3e499fd310b1f615f90932e69a57d2f043a0df417c007dcd40d95e24ed";

/// A page linking to the installer under an address that changes with every
/// visit. Desktop only: a browser may fetch only from sites that allow it.
#[cfg(not(target_arch = "wasm32"))]
const DOWNLOAD_PAGE: &str = "https://www.instaluj.cz/dostihy-3000-deluxe/download";

/// Pictures: (name in the original collections, label, colour keyed out as transparent).
const PICTURES: &[(&str, &str, Option<[u8; 3]>)] = &[
    ("figurka2", "pawn", Some([255, 0, 255])),
    ("zetonZluty", "race", Some([255, 0, 255])),
    ("zetonCerveny", "main_race", Some([255, 0, 255])),
    ("ikony", "icons", Some([255, 255, 255])),
];

/// Sounds: (name in the original collections, label).
const SOUNDS: &[(&str, &str)] = &[
    ("kostky", "sound/dice"),
    ("mince", "sound/coins"),
    ("pokladna", "sound/register"),
    ("karta", "sound/card"),
    ("kun1", "sound/horse"),
    ("GONG3", "sound/gong"),
    ("CLICK11", "sound/click"),
    ("smich1", "sound/laugh"),
];

/// Loads the original game's installer as the [`Original`] resource.
pub struct OriginalPlugin;

impl Plugin for OriginalPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Installer>().init_asset_loader::<InstallerLoader>().add_systems(Startup, start);
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, finish_download);
    }
}

/// The path of an asset from the installer, e.g. `original("horse/napoli")`.
/// Ask for them once the installer has loaded: Bevy loads a file anew for
/// each label asked for while it's still loading.
pub fn original(label: &str) -> String {
    format!("{INSTALLER}#{label}")
}

/// Where the original game's installer is at.
#[derive(Resource)]
pub enum Original {
    /// Being fetched into the assets folder first (desktop).
    #[cfg(not(target_arch = "wasm32"))]
    Downloading(bevy::tasks::Task<Result<(), String>>),
    Loading(Handle<Installer>),
    /// Couldn't be had; the game is played without its art.
    Missing,
}

impl Original {
    /// Its assets can be had (see `original`).
    pub fn loaded(&self, assets: &AssetServer) -> bool {
        matches!(self, Original::Loading(installer) if assets.is_loaded(installer))
    }

    /// Loaded, or known not to be coming.
    pub fn settled(&self, assets: &AssetServer) -> bool {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Original::Downloading(_) => false,
            Original::Loading(installer) => assets.is_loaded(installer) || assets.load_state(installer).is_failed(),
            Original::Missing => true,
        }
    }

    pub fn missing(&self, assets: &AssetServer) -> bool {
        self.settled(assets) && !self.loaded(assets)
    }
}

fn start(mut commands: Commands, assets: Res<AssetServer>) {
    #[cfg(not(target_arch = "wasm32"))]
    if !local_copy().exists() {
        let task = bevy::tasks::IoTaskPool::get().spawn(blocking::unblock(|| download().map_err(|e| e.to_string())));
        commands.insert_resource(Original::Downloading(task));
        return;
    }
    commands.insert_resource(Original::Loading(assets.load(INSTALLER)));
}

#[cfg(not(target_arch = "wasm32"))]
fn finish_download(mut original: ResMut<Original>, assets: Res<AssetServer>) {
    let Original::Downloading(task) = &mut *original else { return };
    let Some(result) = bevy::tasks::futures::check_ready(task) else { return };
    *original = match result {
        Ok(()) => Original::Loading(assets.load(INSTALLER)),
        Err(e) => {
            warn!("{DOWNLOAD_PAGE}: {e}");
            Original::Missing
        }
    };
}

/// The unpacked installer; what's in it are its labeled assets.
#[derive(Asset, TypePath)]
pub struct Installer {
    /// Keeps them loaded while it is, until someone asks for them.
    _assets: Vec<UntypedHandle>,
}

fn known(data: &[u8]) -> bool {
    blake3::hash(data).to_hex().as_str() == INSTALLER_BLAKE3
}

#[cfg(not(target_arch = "wasm32"))]
fn local_copy() -> std::path::PathBuf {
    bevy::asset::io::file::FileAssetReader::get_base_path().join(crate::asset_root()).join(INSTALLER)
}

/// Fetches the installer into the assets folder.
#[cfg(not(target_arch = "wasm32"))]
fn download() -> Result<(), Box<dyn Error + Send + Sync>> {
    let data = link_on_page(DOWNLOAD_PAGE)?;
    if !known(&data) {
        return Err("not the known installer (BLAKE3 differs)".into());
    }
    Ok(std::fs::write(local_copy(), &data)?)
}

#[cfg(not(target_arch = "wasm32"))]
fn link_on_page(page: &str) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    // The site answers 404 to clients that don't look like a browser.
    let get = |url: &str| ureq::get(url).header("User-Agent", "Mozilla/5.0").call()?.body_mut().with_config().limit(64 << 20).read_to_vec();
    let page = String::from_utf8(get(page)?)?;
    // Of its links to the file, the `/dwl/` one redirects to it; the others don't work on their own.
    let url = page
        .split(['"', '\''])
        .find(|s| s.starts_with("https://") && s.contains("/dwl/") && s.ends_with(&format!("/{INSTALLER}")))
        .ok_or("no link to the installer on the page")?;
    Ok(get(url)?)
}

#[derive(Default, TypePath)]
struct InstallerLoader;

impl AssetLoader for InstallerLoader {
    type Asset = Installer;
    type Settings = ();
    type Error = Box<dyn Error + Send + Sync>;

    async fn load(&self, reader: &mut dyn Reader, _: &(), load_context: &mut LoadContext<'_>) -> Result<Installer, Self::Error> {
        let mut installer = Vec::new();
        reader.read_to_end(&mut installer).await?;
        if !known(&installer) {
            warn!("{} isn't the installer this was made for; trying anyway", load_context.path());
        }
        // The installer is a self-extracting zip of DelphiX collections.
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(installer))?;
        let (mut pictures, mut sounds) = (HashMap::new(), HashMap::new());
        for i in 0..zip.len() {
            let mut data = Vec::new();
            std::io::Read::read_to_end(&mut zip.by_index(i)?, &mut data)?;
            collect(&data, b"DELPHIXPICTURECOLLECTION", b"Picture.Data", &mut pictures);
            collect(&data, b"DELPHIXWAVECOLLECTION", b"Wave.WAVE", &mut sounds);
        }

        let mut parts = Vec::new();
        let mut add = |label: String, img: RgbaImage| {
            parts.push(load_context.add_labeled_asset(label, Image::from_dynamic(DynamicImage::ImageRgba8(img), true, RenderAssetUsages::default())).untyped());
        };
        for &(name, label, key) in PICTURES {
            let mut img = bitmap(pictures.get(name).ok_or(format!("picture {name} not found in the installer"))?)?;
            for Rgba(px) in img.pixels_mut() {
                if Some([px[0], px[1], px[2]]) == key {
                    px[3] = 0;
                }
                // The grey pawn is brightened so player colours can tint it.
                if label == "pawn" {
                    for c in &mut px[..3] {
                        *c = (*c as f32 * 1.9).min(255.0) as u8;
                    }
                }
            }
            add(label.to_string(), img);
        }
        // Horse photos are named "01Fantome" … "22Napoli".
        let mut horses = Vec::new();
        for (name, data) in &pictures {
            if let Some(horse) = name.get(2..).filter(|_| name.starts_with(|c: char| c.is_ascii_digit())) {
                horses.push((horse.to_lowercase(), bitmap(data)?));
            }
        }
        cut_out_horses(&mut horses);
        for (horse, img) in horses {
            add(format!("horse/{horse}"), img);
        }
        for &(name, label) in SOUNDS {
            let bytes = sounds.remove(name).ok_or(format!("sound {name} not found in the installer"))?;
            parts.push(load_context.add_labeled_asset(label.to_string(), AudioSource { bytes: bytes.into() }).untyped());
        }
        Ok(Installer { _assets: parts })
    }

    fn extensions(&self) -> &[&str] {
        &["exe"]
    }
}

fn bitmap(bmp: &[u8]) -> Result<RgbaImage, image::ImageError> {
    Ok(image::load_from_memory_with_format(bmp, ImageFormat::Bmp)?.to_rgba8())
}

/// DelphiX collections are Delphi object streams: each item has a `Name` string
/// followed by a binary property holding a BMP (behind a class name) or a WAV.
fn collect(data: &[u8], magic: &[u8], property: &[u8], into: &mut HashMap<String, Vec<u8>>) {
    if !data.get(3..).is_some_and(|d| d.starts_with(magic)) {
        return;
    }
    let mut pos = 0;
    while let Some(at) = find(data, b"\x04Name\x06", pos) {
        let name_start = at + 7;
        let len = data[name_start - 1] as usize;
        let name = data[name_start..name_start + len].iter().map(|&b| b as char).collect::<String>();
        let Some(prop) = find(data, property, name_start) else { return };
        let blob_at = prop + property.len() + 5; // vaBinary tag + u32 length
        let blob_len = u32::from_le_bytes(data[blob_at - 4..blob_at].try_into().unwrap()) as usize;
        let mut blob = &data[blob_at..blob_at + blob_len];
        if blob.first() == Some(&7) {
            // "\x07TBitmap" + u32 size, then the BMP itself
            blob = &blob[12..];
        }
        into.entry(name).or_insert_with(|| blob.to_vec());
        pos = blob_at + blob_len;
    }
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    haystack.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

/// All photos were pasted onto the same piece of wood: where the horses differ,
/// the wood is the colour most photos agree on exactly. Wood-coloured pixels connected to the edge
/// become transparent, starting from the edges and from exact copies of the wood
/// (so similar-coloured spots inside a chestnut horse stay), the outline is softened.
fn cut_out_horses(horses: &mut [(String, RgbaImage)]) {
    let (w, h) = horses[0].1.dimensions();
    let woody = |[r, g, b, _]: [u8; 4]| r > g && g > b && r - b > 60;
    let mut wood = RgbaImage::new(w, h);
    for y in 0..h {
        // (x, colour, how many photos agree)
        let row: Vec<(u32, [u8; 4], usize)> = (0..w)
            .map(|x| {
                let mut counts = HashMap::new();
                for px in horses.iter().map(|(_, img)| img.get_pixel(x, y).0).filter(|&px| woody(px)) {
                    *counts.entry(px).or_insert(0) += 1;
                }
                counts.into_iter().max_by_key(|&(_, n)| n).map_or((x, [0; 4], 0), |(px, n)| (x, px, n))
            })
            .collect();
        // Where most horses overlap, borrow from the nearest reliable spot – the grain runs horizontally.
        let reliable: Vec<_> = row.iter().filter(|r| r.2 >= 3).collect();
        for &(x, px, _) in &row {
            let nearest = reliable.iter().min_by_key(|r| r.0.abs_diff(x)).map_or(px, |r| r.1);
            wood.put_pixel(x, y, Rgba(if row[x as usize].2 >= 3 { px } else { nearest }));
        }
    }
    for (_, img) in horses.iter_mut() {
        let diff = |x: u32, y: u32| {
            let (a, b) = (img.get_pixel(x, y).0, wood.get_pixel(x, y).0);
            (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap()
        };
        let mut wood_mask = vec![false; (w * h) as usize];
        // Seeds: the edges, and pockets (between legs, under the tail) that are exact copies of the wood.
        let mut stack: Vec<(u32, u32)> = (0..w)
            .flat_map(|x| [(x, 0), (x, h - 1)])
            .chain((0..h).flat_map(|y| [(0, y), (w - 1, y)]))
            .chain((0..h).flat_map(|y| (0..w).map(move |x| (x, y))).filter(|&(x, y)| (x.saturating_sub(1)..=(x + 1).min(w - 1)).all(|x| (y.saturating_sub(1)..=(y + 1).min(h - 1)).all(|y| diff(x, y) == 0))))
            .collect();
        while let Some((x, y)) = stack.pop() {
            let i = (y * w + x) as usize;
            if wood_mask[i] || diff(x, y) > 12 {
                continue;
            }
            wood_mask[i] = true;
            stack.extend([(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)].into_iter().filter(|&(x, y)| x < w && y < h));
        }
        let near_wood = |x: u32, y: u32| {
            [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)].into_iter().any(|(x, y)| x < w && y < h && wood_mask[(y * w + x) as usize])
        };
        let alphas: Vec<u8> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| {
                if wood_mask[(y * w + x) as usize] {
                    0
                } else if near_wood(x, y) {
                    (diff(x, y) as u32 * 255 / 40).min(255) as u8
                } else {
                    255
                }
            })
            .collect();
        for (px, alpha) in img.pixels_mut().zip(alphas) {
            px.0[3] = alpha;
        }
    }
}
