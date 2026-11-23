//! Native Windows file dialogs (IFileOpenDialog / IFileSaveDialog), used instead of a dialog plugin.
//! Each call runs on its own STA thread so it never blocks the UI thread or the async runtime.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Folder,
    OpenFile,
    SaveFile,
}

/// Shows a dialog and returns the chosen path, or `None` when cancelled.
/// `filter` is (label, "*.sql;*.txt"); `default_name` is used by Save.
pub fn pick(kind: Kind, title: &str, start_dir: Option<&str>, filter: Option<(&str, &str)>, default_name: Option<&str>) -> Option<String> {
    let title = title.to_string();
    let start_dir = start_dir.map(str::to_string);
    let filter = filter.map(|(a, b)| (a.to_string(), b.to_string()));
    let default_name = default_name.map(str::to_string);
    std::thread::spawn(move || imp::pick(kind, &title, start_dir.as_deref(), filter.as_ref().map(|(a, b)| (a.as_str(), b.as_str())), default_name.as_deref()))
        .join()
        .ok()
        .flatten()
}

#[cfg(windows)]
mod imp {
    use super::Kind;
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED};
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, IShellItem, SHCreateItemFromParsingName,
        FOS_FORCEFILESYSTEM, FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    pub fn pick(kind: Kind, title: &str, start_dir: Option<&str>, filter: Option<(&str, &str)>, default_name: Option<&str>) -> Option<String> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let result = (|| -> windows::core::Result<Option<String>> {
                let dialog: IFileDialog = if kind == Kind::SaveFile {
                    let d: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
                    windows::core::Interface::cast(&d)?
                } else {
                    let d: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
                    windows::core::Interface::cast(&d)?
                };
                let mut opts = dialog.GetOptions()? | FOS_FORCEFILESYSTEM;
                if kind == Kind::Folder {
                    opts |= FOS_PICKFOLDERS;
                }
                dialog.SetOptions(opts)?;
                dialog.SetTitle(&HSTRING::from(title))?;
                if let Some(dir) = start_dir {
                    if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(dir), None) {
                        let _ = dialog.SetFolder(&item);
                    }
                }
                // Keep the strings alive while the dialog runs.
                let label;
                let pattern;
                if let Some((l, p)) = filter {
                    label = HSTRING::from(l);
                    pattern = HSTRING::from(p);
                    let spec = [COMDLG_FILTERSPEC { pszName: PCWSTR(label.as_ptr()), pszSpec: PCWSTR(pattern.as_ptr()) }];
                    dialog.SetFileTypes(&spec)?;
                    if let Some(ext) = p.split(';').next().and_then(|s| s.strip_prefix("*.")) {
                        let _ = dialog.SetDefaultExtension(&HSTRING::from(ext));
                    }
                }
                if let Some(name) = default_name {
                    dialog.SetFileName(&HSTRING::from(name))?;
                }
                if dialog.Show(None).is_err() {
                    return Ok(None); // cancelled
                }
                let item = dialog.GetResult()?;
                let pw = item.GetDisplayName(SIGDN_FILESYSPATH)?;
                let path = pw.to_string().ok();
                CoTaskMemFree(Some(pw.0 as _));
                Ok(path)
            })();
            CoUninitialize();
            result.ok().flatten()
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn pick(_: super::Kind, _: &str, _: Option<&str>, _: Option<(&str, &str)>, _: Option<&str>) -> Option<String> {
        None
    }
}
