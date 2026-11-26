//! Windows: a Start menu shortcut that carries PingCraft's AppUserModelID ([`crate::APP_ID`]). Windows shows toasts
//! only for an ID that a Start menu shortcut carries. The installer makes one; the portable zip has no installer, so
//! on its first run PingCraft makes its own when there is none (maintainer, 2026-11-25).
//!
//! The shortcut is written as a file in the Shell Link format ([MS-SHLLINK]), with the ID in a property store block
//! ([MS-PROPSTORE], `System.AppUserModel.ID`). Writing the bytes needs no COM and so no `unsafe` (the workspace
//! forbids it). The link points at the program through its link info (a local path); its working folder is the
//! program's folder.
//!
//! Not tried on Windows: only built for it and unit-tested elsewhere.

/// `System.AppUserModel.ID`: property 5 of the format {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}.
const APP_USER_MODEL_FORMAT: [u8; 16] = guid(0x9f4c_2855, 0x9f79, 0x4b39, [0xa8, 0xd0, 0xe1, 0xd4, 0x2d, 0xe1, 0xd5, 0xf3]);
const APP_USER_MODEL_ID: u32 = 5;
/// The Shell Link class, {00021401-0000-0000-C000-000000000046}.
const SHELL_LINK_CLSID: [u8; 16] = guid(0x0002_1401, 0, 0, [0xc0, 0, 0, 0, 0, 0, 0, 0x46]);

const HAS_LINK_INFO: u32 = 0x02;
const HAS_WORKING_DIR: u32 = 0x10;
const IS_UNICODE: u32 = 0x80;
const PROPERTY_STORE_BLOCK: u32 = 0xa000_0009;
/// "1SPS", a serialized property storage's version.
const PROPERTY_STORAGE_VERSION: u32 = 0x5350_5331;
const VT_LPWSTR: u16 = 0x001f;
const DRIVE_FIXED: u32 = 3;
const SW_SHOWNORMAL: u32 = 1;

/// A GUID as it is stored: the first three fields little-endian, the last eight bytes as they are.
const fn guid(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> [u8; 16] {
    let a = d1.to_le_bytes();
    let b = d2.to_le_bytes();
    let c = d3.to_le_bytes();
    [a[0], a[1], a[2], a[3], b[0], b[1], c[0], c[1], d4[0], d4[1], d4[2], d4[3], d4[4], d4[5], d4[6], d4[7]]
}

fn utf16(s: &str) -> impl Iterator<Item = u8> + '_ {
    s.encode_utf16().flat_map(u16::to_le_bytes)
}

fn u32_len(n: usize) -> Result<u32, String> {
    u32::try_from(n).map_err(|_| "the shortcut would be too big".to_owned())
}

/// The link info: the target as a local path on a fixed drive, in the system code page (non-ASCII as `?`) and in
/// UTF-16, which Windows prefers when it is there.
fn link_info(target: &str) -> Result<Vec<u8>, String> {
    const HEADER: usize = 0x24;
    let volume: Vec<u8> =
        [0x11_u32.to_le_bytes(), DRIVE_FIXED.to_le_bytes(), 0_u32.to_le_bytes(), 0x10_u32.to_le_bytes()].concat().into_iter().chain([0]).collect();
    let ansi: Vec<u8> = target.chars().map(|c| if c.is_ascii() { c as u8 } else { b'?' }).chain([0]).collect();
    let wide: Vec<u8> = utf16(target).chain([0, 0]).collect();
    let volume_at = HEADER;
    let base_at = volume_at + volume.len();
    let suffix_at = base_at + ansi.len();
    let wide_at = suffix_at + 1;
    let wide_suffix_at = wide_at + wide.len();
    let size = wide_suffix_at + 2;
    let mut out = Vec::with_capacity(size);
    for field in [size, HEADER, 1, volume_at, base_at, 0, suffix_at, wide_at, wide_suffix_at] {
        out.extend(u32_len(field)?.to_le_bytes());
    }
    out.extend(volume);
    out.extend(ansi);
    out.push(0);
    out.extend(wide);
    out.extend([0, 0]);
    Ok(out)
}

/// The property store block with the AppUserModelID.
fn property_store(app_id: &str) -> Result<Vec<u8>, String> {
    let chars = app_id.encode_utf16().count() + 1;
    let mut text: Vec<u8> = utf16(app_id).chain([0, 0]).collect();
    text.resize(text.len().div_ceil(4) * 4, 0);
    let mut value = Vec::new();
    value.extend(APP_USER_MODEL_ID.to_le_bytes());
    value.push(0);
    value.extend(VT_LPWSTR.to_le_bytes());
    value.extend(0_u16.to_le_bytes());
    value.extend(u32_len(chars)?.to_le_bytes());
    value.extend(text);
    let mut storage = Vec::new();
    storage.extend(PROPERTY_STORAGE_VERSION.to_le_bytes());
    storage.extend(APP_USER_MODEL_FORMAT);
    storage.extend(u32_len(value.len() + 4)?.to_le_bytes());
    storage.extend(value);
    storage.extend(0_u32.to_le_bytes());
    let mut block = Vec::new();
    let block_size = 8 + 4 + storage.len() + 4;
    block.extend(u32_len(block_size)?.to_le_bytes());
    block.extend(PROPERTY_STORE_BLOCK.to_le_bytes());
    block.extend(u32_len(storage.len() + 4)?.to_le_bytes());
    block.extend(storage);
    block.extend(0_u32.to_le_bytes());
    Ok(block)
}

/// A shortcut to `target` (an absolute local path), starting in `working_dir`, carrying `app_id`.
pub(crate) fn link_bytes(target: &str, working_dir: &str, app_id: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    out.extend(0x4c_u32.to_le_bytes());
    out.extend(SHELL_LINK_CLSID);
    out.extend((HAS_LINK_INFO | HAS_WORKING_DIR | IS_UNICODE).to_le_bytes());
    // File attributes, three times, the file size and the icon index: none.
    out.extend([0_u8; 4 + 24 + 4 + 4]);
    out.extend(SW_SHOWNORMAL.to_le_bytes());
    // Hot key and three reserved fields.
    out.extend([0_u8; 2 + 2 + 4 + 4]);
    out.extend(link_info(target)?);
    let dir_chars = u16::try_from(working_dir.encode_utf16().count()).map_err(|_| "the folder's name is too long".to_owned())?;
    out.extend(dir_chars.to_le_bytes());
    out.extend(utf16(working_dir));
    out.extend(property_store(app_id)?);
    // The terminal block.
    out.extend(0_u32.to_le_bytes());
    Ok(out)
}

/// Makes the Start menu shortcut when the user's and the machine's Start menus have none named PingCraft. Returns
/// whether it made one. Never fails loudly: the caller says so once and goes on.
#[cfg(target_os = "windows")]
pub fn ensure_start_menu_shortcut() -> Result<bool, String> {
    use std::path::PathBuf;
    const PROGRAMS: &str = r"Microsoft\Windows\Start Menu\Programs";
    let name = format!("{}.lnk", crate::APP_NAME);
    let user = PathBuf::from(std::env::var_os("APPDATA").ok_or("no APPDATA folder")?).join(PROGRAMS);
    let machine = std::env::var_os("ProgramData").map(|p| PathBuf::from(p).join(PROGRAMS).join(&name));
    if user.join(&name).exists() || machine.is_some_and(|m| m.exists()) {
        return Ok(false);
    }
    let exe = std::env::current_exe().map_err(|e| format!("where PingCraft runs from: {}", e.kind()))?;
    let exe = exe.to_str().ok_or("PingCraft's path is not Unicode")?;
    let exe = exe.strip_prefix(r"\\?\").unwrap_or(exe);
    let dir = exe.rsplit_once('\\').map_or(exe, |(dir, _)| dir);
    let bytes = link_bytes(exe, dir, crate::APP_ID)?;
    std::fs::create_dir_all(&user).map_err(|e| format!("the Start menu folder: {}", e.kind()))?;
    let part = user.join(format!("{name}.part"));
    std::fs::write(&part, bytes).map_err(|e| format!("writing the shortcut: {}", e.kind()))?;
    std::fs::rename(&part, user.join(&name)).map_err(|e| format!("writing the shortcut: {}", e.kind()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::link_bytes;

    fn u32_at(b: &[u8], at: usize) -> usize {
        u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as usize
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn a_shortcut_carries_the_app_id_and_points_at_the_program() {
        let target = r"C:\Users\Ana\Apps\PingCraft\pingcraft.exe";
        let b = link_bytes(target, r"C:\Users\Ana\Apps\PingCraft", crate::APP_ID).unwrap();
        // The header: its size, the Shell Link class, and link info, a working folder and Unicode strings.
        assert_eq!(u32_at(&b, 0), 0x4c);
        assert_eq!(&b[4..20], &[0x01, 0x14, 0x02, 0, 0, 0, 0, 0, 0xc0, 0, 0, 0, 0, 0, 0, 0x46]);
        assert_eq!(u32_at(&b, 20), 0x02 | 0x10 | 0x80);
        // The link info follows the header; its sizes add up, and it names the program in both encodings.
        let info = &b[0x4c..0x4c + u32_at(&b, 0x4c)];
        assert_eq!(u32_at(info, 4), 0x24, "header with the Unicode offsets");
        let base = u32_at(info, 16);
        assert_eq!(&info[base..base + target.len()], target.as_bytes());
        let wide: Vec<u8> = target.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let wide_at = u32_at(info, 28);
        assert_eq!(&info[wide_at..wide_at + wide.len()], &wide[..]);
        // The working folder, then the property store block with the ID as a wide string.
        let dir_at = 0x4c + info.len();
        assert_eq!(u16::from_le_bytes([b[dir_at], b[dir_at + 1]]) as usize, r"C:\Users\Ana\Apps\PingCraft".len());
        let block_at = dir_at + 2 + 2 * r"C:\Users\Ana\Apps\PingCraft".len();
        assert_eq!(u32_at(&b, block_at + 4), 0xa000_0009);
        assert_eq!(block_at + u32_at(&b, block_at) + 4, b.len(), "the block, then the terminal block, end the file");
        assert_eq!(u32_at(&b, b.len() - 4), 0);
        let id: Vec<u8> = crate::APP_ID.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let id_at = find(&b, &id).expect("the ID is in the file");
        assert!(id_at > block_at);
        assert_eq!(u32_at(&b, id_at - 4), crate::APP_ID.len() + 1, "its length counts the closing nul");
        assert_eq!(&b[id_at - 8..id_at - 4], &[0x1f, 0, 0, 0], "a VT_LPWSTR");
        assert_eq!(u32_at(&b, id_at - 13), 5, "System.AppUserModel.ID is property 5");
    }

    #[test]
    fn a_path_outside_ascii_is_kept_in_utf16() {
        let target = r"C:\Brugere\Søren\pingcraft.exe";
        let b = link_bytes(target, r"C:\Brugere\Søren", crate::APP_ID).unwrap();
        let wide: Vec<u8> = target.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert!(find(&b, &wide).is_some());
        assert!(find(&b, br"C:\Brugere\S?ren\pingcraft.exe").is_some(), "the code page path stands in a ? for ø");
    }
}
