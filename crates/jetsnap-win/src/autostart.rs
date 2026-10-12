//! 开机自启（仅 Windows）：HKCU\Software\Microsoft\Windows\CurrentVersion\Run。

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "JetSnap";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 设置 / 取消开机自启。`exe_path` 为当前可执行文件绝对路径。
pub fn set_autostart(enabled: bool, exe_path: &str) -> Result<(), String> {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE,
        REG_SZ,
    };
    use windows::core::PCWSTR;

    unsafe {
        let subkey = wide(RUN_KEY);
        let mut hkey = HKEY_CURRENT_USER;
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            None,
            KEY_SET_VALUE,
            &mut hkey,
        );
        if status.0 != 0 {
            return Err(format!("打开注册表 Run 键失败: 0x{:08X}", status.0));
        }
        let name = wide(VALUE_NAME);
        let result = (|| {
            if enabled {
                // 引号包裹，兼容含空格路径
                let data = format!("\"{exe_path}\"");
                let data_wide = wide(&data);
                // REG_SZ 需要 UTF-16LE 字节 + 终止符
                let mut le: Vec<u8> = Vec::with_capacity(data_wide.len() * 2);
                for u in &data_wide {
                    le.extend_from_slice(&u.to_le_bytes());
                }
                let status = RegSetValueExW(
                    hkey,
                    PCWSTR(name.as_ptr()),
                    None,
                    REG_SZ,
                    Some(&le),
                    le.len() as u32,
                );
                if status.0 != 0 {
                    Err(format!("写入自启项失败: 0x{:08X}", status.0))
                } else {
                    Ok(())
                }
            } else {
                let status = RegDeleteValueW(hkey, PCWSTR(name.as_ptr()));
                // 值不存在视为已关闭，幂等成功
                if status.0 == 0 || status.0 == 2 {
                    Ok(())
                } else {
                    Err(format!("删除自启项失败: 0x{:08X}", status.0))
                }
            }
        })();
        let _ = RegCloseKey(hkey);
        result
    }
}
