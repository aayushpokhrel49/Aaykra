use client::{LEGACY_ZED_URL_SCHEME, ZED_URL_SCHEME};
use gpui::{AsyncApp, actions};

actions!(
    cli,
    [
        /// Registers the aaykra:// URL scheme handler.
        RegisterAaykraScheme
    ]
);

pub async fn register_aaykra_scheme(cx: &AsyncApp) -> anyhow::Result<()> {
    cx.update(|cx| cx.register_url_scheme(ZED_URL_SCHEME)).await?;
    cx.update(|cx| cx.register_url_scheme(LEGACY_ZED_URL_SCHEME))
        .await
}
