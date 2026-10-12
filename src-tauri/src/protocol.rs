//! snapshot:// 自定义协议（开发计划 3.3-⑤）：
//! 4K BGRA 约 33MB，绝不能走 invoke 序列化通道。此处从会话内存直出 JPEG。

use tauri::http::Response;
use tauri::{Manager, UriSchemeContext};

use crate::state::AppState;

pub fn handle_snapshot<R: tauri::Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: tauri::http::Request<Vec<u8>>,
) -> Response<std::borrow::Cow<'static, [u8]>> {
    let uri = request.uri().to_string();
    let id = snapshot_id_from_uri(&uri);

    let state = ctx.app_handle().state::<AppState>();
    let jpeg = {
        let guard = state.session.lock().unwrap();
        match guard.as_ref() {
            Some(s) if Some(s.id) == id => Some(s.jpeg.clone()),
            _ => None,
        }
    };

    match jpeg {
        Some(bytes) => Response::builder()
            .status(200)
            .header("Content-Type", "image/jpeg")
            .header("Cache-Control", "no-store")
            .body(std::borrow::Cow::Owned(bytes))
            .expect("构建 snapshot 响应失败"),
        None => Response::builder()
            .status(404)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(std::borrow::Cow::Borrowed(b"snapshot not found".as_slice()))
            .expect("构建 404 响应失败"),
    }
}

/// 从 URI 中提取第一段连续数字作为快照 id。
///
/// Windows WebView2 可能将自定义协议请求改写为
/// `http://snapshot.localhost/<id>`，Linux/Android 保持 `snapshot://<id>`，
/// 提取数字段两种形态通吃。
fn snapshot_id_from_uri(uri: &str) -> Option<u32> {
    let digits: String = uri
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::snapshot_id_from_uri;

    #[test]
    fn extracts_from_both_uri_forms() {
        assert_eq!(snapshot_id_from_uri("snapshot://42"), Some(42));
        assert_eq!(snapshot_id_from_uri("http://snapshot.localhost/7"), Some(7));
        assert_eq!(snapshot_id_from_uri("snapshot://localhost/13"), Some(13));
    }

    #[test]
    fn no_digits_is_none() {
        assert_eq!(snapshot_id_from_uri("snapshot://abc"), None);
        assert_eq!(snapshot_id_from_uri("about:blank"), None);
    }
}
