//! System font enumeration for the font pickers.
//! Windows: DirectWrite IDWriteFactory::GetSystemFontCollection.
// Other platforms fall back to a small curated list (CoreText/fontconfig
// enumeration can be added behind their own cfg gates later).

fn default_fonts() -> Vec<String> {
    vec![
        "Segoe UI".into(),
        "Arial".into(),
        "Calibri".into(),
        "Consolas".into(),
        "Courier New".into(),
        "Georgia".into(),
        "Tahoma".into(),
        "Times New Roman".into(),
        "Verdana".into(),
    ]
}

#[cfg(target_os = "windows")]
pub fn system_fonts() -> Vec<String> {
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection, DWRITE_FACTORY_TYPE_SHARED,
    };
    let mut out: Vec<String> = Vec::new();
    unsafe {
        let factory: IDWriteFactory = match DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) {
            Ok(f) => f,
            Err(_) => return default_fonts(),
        };
        let mut collection: Option<IDWriteFontCollection> = None;
        if factory.GetSystemFontCollection(&mut collection, false).is_err() {
            return default_fonts();
        }
        let collection = match collection {
            Some(c) => c,
            None => return default_fonts(),
        };
        let count = collection.GetFontFamilyCount();
        for i in 0..count {
            let family = match collection.GetFontFamily(i) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let names = match family.GetFamilyNames() {
                Ok(n) => n,
                Err(_) => continue,
            };
            let len = names.GetStringLength(0).unwrap_or(0);
            if len == 0 {
                continue;
            }
            let mut buf = vec![0u16; (len + 1) as usize];
            if names.GetString(0, &mut buf).is_ok() {
                let s = String::from_utf16_lossy(&buf[..len as usize]);
                let s = s.trim().to_string();
                if !s.is_empty() && !s.starts_with('@') {
                    out.push(s);
                }
            }
        }
    }
    out.sort_by_key(|s| s.to_lowercase());
    out.dedup();
    if out.is_empty() {
        default_fonts()
    } else {
        out
    }
}

#[cfg(not(target_os = "windows"))]
pub fn system_fonts() -> Vec<String> {
    default_fonts()
}
