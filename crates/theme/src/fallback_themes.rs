use std::sync::Arc;

use gpui::{FontStyle, FontWeight, HighlightStyle, Hsla, WindowBackgroundAppearance, rgba};

use crate::{
    AccentColors, Appearance, DEFAULT_DARK_THEME, PlayerColors, StatusColors,
    StatusColorsRefinement, SyntaxTheme, SystemColors, Theme, ThemeColors, ThemeColorsRefinement,
    ThemeFamily, ThemeStyles, default_color_scales,
};

/// The default theme family for Aaykra.
///
/// This is used to construct the default theme fallback values, as well as to
/// have a theme available at compile time for tests. Its colors mirror
/// `assets/themes/aaykra/aaykra.json`, which is what ships in release builds.
pub fn aaykra_default_themes() -> ThemeFamily {
    ThemeFamily {
        id: "aaykra-default".to_string(),
        name: "Aaykra".into(),
        author: "Aaykra".into(),
        themes: vec![aaykra_default_dark()],
        scales: default_color_scales(),
    }
}

// If a theme customizes a foreground version of a status color, but does not
// customize the background color, then use a partly-transparent version of the
// foreground color for the background color.
/// Applies default status color backgrounds from their foreground counterparts.
pub fn apply_status_color_defaults(status: &mut StatusColorsRefinement) {
    for (fg_color, bg_color) in [
        (&status.deleted, &mut status.deleted_background),
        (&status.created, &mut status.created_background),
        (&status.modified, &mut status.modified_background),
        (&status.conflict, &mut status.conflict_background),
        (&status.error, &mut status.error_background),
        (&status.hidden, &mut status.hidden_background),
    ] {
        if bg_color.is_none()
            && let Some(fg_color) = fg_color
        {
            *bg_color = Some(fg_color.opacity(0.25));
        }
    }
}

/// Applies default theme color values derived from player colors.
pub fn apply_theme_color_defaults(
    theme_colors: &mut ThemeColorsRefinement,
    player_colors: &PlayerColors,
) {
    if theme_colors.element_selection_background.is_none() {
        let mut selection = player_colors.local().selection;
        if selection.a == 1.0 {
            selection.a = 0.25;
        }
        theme_colors.element_selection_background = Some(selection);
    }
}

/// Builds a color from a `0xRRGGBBAA` literal, matching the encoding used by
/// the bundled theme JSON.
fn color(hex: u32) -> Hsla {
    Hsla::from(rgba(hex))
}

const ADDED_COLOR: Hsla = Hsla {
    h: 134. / 360.,
    s: 0.55,
    l: 0.40,
    a: 1.0,
};
const WORD_ADDED_COLOR: Hsla = Hsla {
    h: 134. / 360.,
    s: 0.55,
    l: 0.40,
    a: 0.35,
};
const MODIFIED_COLOR: Hsla = Hsla {
    h: 48. / 360.,
    s: 0.76,
    l: 0.47,
    a: 1.0,
};
const REMOVED_COLOR: Hsla = Hsla {
    h: 350. / 360.,
    s: 0.88,
    l: 0.25,
    a: 1.0,
};
const WORD_DELETED_COLOR: Hsla = Hsla {
    h: 350. / 360.,
    s: 0.88,
    l: 0.25,
    a: 0.80,
};

pub(crate) fn aaykra_default_dark() -> Theme {
    let player = PlayerColors::dark();
    Theme {
        id: "aaykra".to_string(),
        name: DEFAULT_DARK_THEME.into(),
        appearance: Appearance::Dark,
        styles: ThemeStyles {
            window_background_appearance: WindowBackgroundAppearance::Opaque,
            system: SystemColors::default(),
            accents: AccentColors::dark(),
            colors: ThemeColors {
                border: color(0x2a2a2aff),
                border_variant: color(0x333333ff),
                border_focused: color(0x228df255),
                border_selected: color(0x228df2ff),
                border_transparent: color(0x00000000),
                border_disabled: color(0x2a2a2a88),
                elevated_surface_background: color(0x0f0f0fff),
                surface_background: color(0x141414ff),
                background: color(0x181818ff),
                element_background: color(0x1e1e1eff),
                element_hover: color(0x252525ff),
                element_active: color(0x2f2f2fff),
                element_selected: color(0x2a2a2aff),
                element_disabled: color(0x1a1a1aff),
                element_selection_background: color(0x003362ff).opacity(0.25),
                drop_target_background: color(0x228df215),
                drop_target_border: color(0xeeeeecff),
                ghost_element_background: color(0x00000000),
                ghost_element_hover: color(0xffffff0d),
                ghost_element_active: color(0xffffff1a),
                ghost_element_selected: color(0x228df218),
                ghost_element_disabled: color(0x14141488),
                text: color(0xd6d6ddff),
                text_muted: color(0x8c8c8cff),
                text_placeholder: color(0x505050ff),
                text_disabled: color(0x404040ff),
                text_accent: color(0x228df2ff),
                icon: color(0xd6d6ddff),
                icon_muted: color(0x505050ff),
                icon_disabled: color(0x3a3a3aff),
                icon_placeholder: color(0x505050ff),
                icon_accent: color(0x228df2ff),
                debugger_accent: color(0xdc3e42ff),
                status_bar_background: color(0x141414ff),
                title_bar_background: color(0x141414ff),
                title_bar_inactive_background: color(0x222221ff),
                toolbar_background: color(0x181818ff),
                tab_bar_background: color(0x141414ff),
                tab_inactive_background: color(0x141414ff),
                tab_active_background: color(0x1a1a1aff),
                search_match_background: color(0x228df240),
                search_active_match_background: color(0x222221ff),
                panel_background: color(0x141414ff),
                panel_focused_border: color(0x228df255),
                panel_indent_guide: color(0xfefef31b),
                panel_indent_guide_hover: color(0xfffaed2d),
                panel_indent_guide_active: color(0xfffaed2d),
                panel_overlay_background: color(0x191918ff),
                panel_overlay_hover: color(0x2a2a28ff),
                pane_focused_border: color(0x228df230),
                pane_group_border: color(0x3b3a37ff),
                scrollbar_thumb_background: color(0x3a3a3acc),
                scrollbar_thumb_hover_background: color(0x3a3a3aff),
                scrollbar_thumb_active_background: color(0xfbfbeb23),
                scrollbar_thumb_border: color(0x00000000),
                scrollbar_track_background: color(0x181818ff),
                scrollbar_track_border: color(0x2a2a2aff),
                minimap_thumb_background: color(0xf6f6f513).opacity(0.7),
                minimap_thumb_hover_background: color(0xfefef31b).opacity(0.7),
                minimap_thumb_active_background: color(0xfbfbeb23).opacity(0.7),
                minimap_thumb_border: color(0x00000000),
                editor_foreground: color(0xd6d6ddff),
                editor_background: color(0x181818ff),
                editor_gutter_background: color(0x181818ff),
                editor_subheader_background: color(0x141414ff),
                editor_active_line_background: color(0x292929ff),
                editor_highlighted_line_background: color(0x228df210),
                editor_debugger_active_line_background: color(0xffaa001e),
                editor_line_number: color(0x505050ff),
                editor_hover_line_number: color(0xfffffded),
                editor_active_line_number: color(0xffffffff),
                editor_invisible: color(0x333333ff),
                editor_wrap_guide: color(0x2a2a2aff),
                editor_active_wrap_guide: color(0x3a3a3aff),
                editor_indent_guide: color(0xfefef31b),
                editor_indent_guide_active: color(0xfffaed2d),
                editor_document_highlight_read_background: color(0x228df222),
                editor_document_highlight_write_background: color(0x228df233),
                editor_document_highlight_bracket_background: color(0x44ffaa4b),
                editor_diff_hunk_added_background: ADDED_COLOR.opacity(0.12),
                editor_diff_hunk_added_hollow_background: ADDED_COLOR.opacity(0.06),
                editor_diff_hunk_added_hollow_border: ADDED_COLOR.opacity(0.36),
                editor_diff_hunk_deleted_background: REMOVED_COLOR.opacity(0.12),
                editor_diff_hunk_deleted_hollow_background: REMOVED_COLOR.opacity(0.06),
                editor_diff_hunk_deleted_hollow_border: REMOVED_COLOR.opacity(0.36),
<<<<<<< 5a0780efc8eaf2484f8abfa16efc6194cfe72888
                terminal_background: color(0x141414ff),
                terminal_ansi_background: color(0x111110ff),
                terminal_foreground: color(0xd6d6ddff),
                terminal_bright_foreground: color(0xffffffff),
                terminal_dim_foreground: color(0x8c8c8cff),
                terminal_ansi_black: color(0x676767ff),
                terminal_ansi_red: color(0xf14c4cff),
                terminal_ansi_green: color(0x15ac91ff),
                terminal_ansi_yellow: color(0xe5b95cff),
                terminal_ansi_blue: color(0x4c9df3ff),
                terminal_ansi_magenta: color(0xe567dcff),
                terminal_ansi_cyan: color(0x75d3baff),
                terminal_ansi_white: color(0xd6d6ddff),
                terminal_ansi_bright_black: color(0x888888ff),
                terminal_ansi_bright_red: color(0xf76c6cff),
                terminal_ansi_bright_green: color(0x2cc9aeff),
                terminal_ansi_bright_yellow: color(0xf0c97aff),
                terminal_ansi_bright_blue: color(0x70b5f9ff),
                terminal_ansi_bright_magenta: color(0xec85e8ff),
                terminal_ansi_bright_cyan: color(0x93ddc9ff),
                terminal_ansi_bright_white: color(0xffffffff),
                terminal_ansi_dim_black: color(0x000000b3),
                terminal_ansi_dim_red: color(0xe5484dff),
                terminal_ansi_dim_green: color(0x30a46cff),
                terminal_ansi_dim_yellow: color(0xffe629ff),
                terminal_ansi_dim_blue: color(0x0090ffff),
                terminal_ansi_dim_magenta: color(0x6e56cfff),
                terminal_ansi_dim_cyan: color(0x00a2c7ff),
                terminal_ansi_dim_white: color(0x6f6d66ff),
                link_text_hover: color(0x4ca8f5ff),
=======

                terminal_background: bg,
                // todo("Use one colors for terminal")
                terminal_ansi_background: crate::black().dark().step_12(),
                terminal_foreground: crate::white().dark().step_12(),
                terminal_bright_foreground: crate::white().dark().step_11(),
                terminal_dim_foreground: crate::white().dark().step_10(),
                terminal_ansi_black: crate::black().dark().step_12(),
                terminal_ansi_red: crate::red().dark().step_11(),
                terminal_ansi_green: crate::green().dark().step_11(),
                terminal_ansi_yellow: crate::yellow().dark().step_11(),
                terminal_ansi_blue: crate::blue().dark().step_11(),
                terminal_ansi_magenta: crate::violet().dark().step_11(),
                terminal_ansi_cyan: crate::cyan().dark().step_11(),
                terminal_ansi_white: crate::neutral().dark().step_12(),
                terminal_ansi_bright_black: crate::black().dark().step_11(),
                terminal_ansi_bright_red: crate::red().dark().step_10(),
                terminal_ansi_bright_green: crate::green().dark().step_10(),
                terminal_ansi_bright_yellow: crate::yellow().dark().step_10(),
                terminal_ansi_bright_blue: crate::blue().dark().step_10(),
                terminal_ansi_bright_magenta: crate::violet().dark().step_10(),
                terminal_ansi_bright_cyan: crate::cyan().dark().step_10(),
                terminal_ansi_bright_white: crate::neutral().dark().step_11(),
                terminal_ansi_dim_black: crate::black().dark().step_10(),
                terminal_ansi_dim_red: crate::red().dark().step_9(),
                terminal_ansi_dim_green: crate::green().dark().step_9(),
                terminal_ansi_dim_yellow: crate::yellow().dark().step_9(),
                terminal_ansi_dim_blue: crate::blue().dark().step_9(),
                terminal_ansi_dim_magenta: crate::violet().dark().step_9(),
                terminal_ansi_dim_cyan: crate::cyan().dark().step_9(),
                terminal_ansi_dim_white: crate::neutral().dark().step_10(),
                panel_background: bg,
                panel_focused_border: blue,
                panel_indent_guide: hsla(228. / 360., 8. / 100., 25. / 100., 1.),
                panel_indent_guide_hover: hsla(225. / 360., 13. / 100., 12. / 100., 1.),
                panel_indent_guide_active: hsla(225. / 360., 13. / 100., 12. / 100., 1.),
                panel_overlay_background: bg,
                panel_overlay_hover: hover,
                pane_focused_border: blue,
                pane_group_border: hsla(225. / 360., 13. / 100., 12. / 100., 1.),
                scrollbar_thumb_background: gpui::transparent_black(),
                scrollbar_thumb_hover_background: hover,
                scrollbar_thumb_active_background: hsla(
                    225.0 / 360.,
                    11.8 / 100.,
                    26.7 / 100.,
                    1.0,
                ),
                scrollbar_thumb_border: hsla(228. / 360., 8. / 100., 25. / 100., 1.),
                scrollbar_track_background: gpui::transparent_black(),
                scrollbar_track_border: hsla(228. / 360., 8. / 100., 25. / 100., 1.),
                minimap_thumb_background: hsla(225.0 / 360., 11.8 / 100., 26.7 / 100., 0.7),
                minimap_thumb_hover_background: hsla(225.0 / 360., 11.8 / 100., 26.7 / 100., 0.7),
                minimap_thumb_active_background: hsla(225.0 / 360., 11.8 / 100., 26.7 / 100., 0.7),
                minimap_thumb_border: hsla(228. / 360., 8. / 100., 25. / 100., 1.),
                editor_foreground: hsla(218. / 360., 14. / 100., 71. / 100., 1.),
                editor_code_lens_foreground: None,
                link_text_hover: blue,
>>>>>>> 2abf58ae93479ba0ebd02d357f3332c9d40baf50
                version_control_added: ADDED_COLOR,
                version_control_deleted: REMOVED_COLOR,
                version_control_modified: MODIFIED_COLOR,
                version_control_renamed: MODIFIED_COLOR,
                version_control_conflict: color(0xffe0c2ff),
                version_control_ignored: color(0xeeeeeeff),
                version_control_word_added: WORD_ADDED_COLOR,
                version_control_word_deleted: WORD_DELETED_COLOR,
                version_control_conflict_marker_ours: color(0x33b074ff).opacity(0.5),
                version_control_conflict_marker_theirs: color(0x3b9effff).opacity(0.5),
            },
            status: StatusColors {
                conflict: color(0xe5b95cff),
                conflict_background: color(0x251e0fff),
                conflict_border: color(0x40320aff),
                created: color(0x15ac91ff),
                created_background: color(0x102520ff),
                created_border: color(0x1a3830ff),
                deleted: color(0xf14c4cff),
                deleted_background: color(0x2a1a1aff),
                deleted_border: color(0x5a2020ff),
                error: color(0xf14c4cff),
                error_background: color(0x2a1a1aff),
                error_border: color(0x5a2020ff),
                hidden: color(0x6f6d66ff),
                hidden_background: color(0x6f6d66ff),
                hidden_border: color(0x6f6d66ff),
                hint: color(0x505050ff),
                hint_background: color(0x1e1e1eff),
                hint_border: color(0x2a2a2aff),
                ignored: color(0x505050ff),
                ignored_background: color(0x6f6d66ff),
                ignored_border: color(0x6f6d66ff),
                info: color(0x228df2ff),
                info_background: color(0x142035ff),
                info_border: color(0x1e3a5aff),
                modified: color(0x228df2ff),
                modified_background: color(0x142035ff),
                modified_border: color(0x1e3a5aff),
                predictive: color(0x505050ff),
                predictive_background: color(0x1a1a1aff),
                predictive_border: color(0x2a2a2aff),
                renamed: color(0x83d6c5ff),
                renamed_background: color(0x152525ff),
                renamed_border: color(0x1e3a3aff),
                success: color(0x15ac91ff),
                success_background: color(0x102520ff),
                success_border: color(0x1a3830ff),
                unreachable: color(0x7c7b74ff),
                unreachable_background: color(0x7c7b74ff),
                unreachable_border: color(0x7c7b74ff),
                warning: color(0xe5b95cff),
                warning_background: color(0x251e0fff),
                warning_border: color(0x40320aff),
            },
            player,
            syntax: Arc::new(SyntaxTheme::new(vec![
                ("attribute".into(), color(0xaaa0faff).into()),
                ("boolean".into(), keyword_style(color(0x83d6c5ff))),
                ("comment".into(), comment_style(color(0x6d6d6dff))),
                ("comment.doc".into(), comment_style(color(0x6d6d6dff))),
                ("constant".into(), color(0xaaa0faff).into()),
                ("constructor".into(), color(0x87c3ffff).into()),
                ("embedded".into(), HighlightStyle::default()),
                (
                    "emphasis".into(),
                    HighlightStyle {
                        color: Some(color(0xd6d6ddff)),
                        font_style: Some(FontStyle::Italic),
                        ..HighlightStyle::default()
                    },
                ),
                (
                    "emphasis.strong".into(),
                    HighlightStyle {
                        color: Some(color(0xd6d6ddff)),
                        font_weight: Some(FontWeight::BOLD),
                        ..HighlightStyle::default()
                    },
                ),
                ("enum".into(), color(0x87c3ffff).into()),
                ("function".into(), color(0xefb080ff).into()),
                ("hint".into(), color(0x505050ff).into()),
                ("keyword".into(), keyword_style(color(0x83d6c5ff))),
                (
                    "label".into(),
                    HighlightStyle {
                        color: Some(color(0xd6d6ddff)),
                        font_weight: Some(FontWeight::BOLD),
                        ..HighlightStyle::default()
                    },
                ),
                (
                    "link.text".into(),
                    color(0x4ca8f5ff).into(),
                ),
                (
                    "link.uri".into(),
                    HighlightStyle {
                        color: Some(color(0x4ca8f5ff)),
                        font_style: Some(FontStyle::Italic),
                        ..HighlightStyle::default()
                    },
                ),
                ("link_text".into(), color(0x4ca8f5ff).into()),
                (
                    "link_uri".into(),
                    HighlightStyle {
                        color: Some(color(0x4ca8f5ff)),
                        font_style: Some(FontStyle::Italic),
                        ..HighlightStyle::default()
                    },
                ),
                ("number".into(), color(0x87c3ffff).into()),
                ("operator".into(), color(0xd6d6ddff).into()),
                ("predictive".into(), color(0x505050ff).into()),
                ("preproc".into(), color(0x83d6c5ff).into()),
                ("primary".into(), HighlightStyle::default()),
                ("property".into(), color(0xd6d6ddff).into()),
                ("punctuation".into(), HighlightStyle::default()),
                ("punctuation.bracket".into(), color(0xd6d6ddff).into()),
                ("punctuation.delimiter".into(), HighlightStyle::default()),
                (
                    "punctuation.list_marker".into(),
                    color(0x83d6c5ff).into(),
                ),
                ("punctuation.special".into(), HighlightStyle::default()),
                ("string".into(), color(0xe394dcff).into()),
                ("string.escape".into(), HighlightStyle::default()),
                ("string.regex".into(), HighlightStyle::default()),
                ("string.special".into(), color(0xe394dcff).into()),
                (
                    "string.special.symbol".into(),
                    color(0xaaa0faff).into(),
                ),
                ("tag".into(), color(0x87c3ffff).into()),
                ("text.literal".into(), color(0xe394dcff).into()),
                (
                    "title".into(),
                    HighlightStyle {
                        color: Some(color(0x87c3ffff)),
                        font_weight: Some(FontWeight::BOLD),
                        ..HighlightStyle::default()
                    },
                ),
                ("type".into(), color(0x87c3ffff).into()),
                ("variable".into(), color(0xd6d6ddff).into()),
                ("variable.special".into(), HighlightStyle::default()),
                ("variant".into(), HighlightStyle::default()),
                ("diff.plus".into(), color(0x15ac91ff).into()),
                ("diff.minus".into(), color(0xf14c4cff).into()),
            ])),
        },
    }
}

fn comment_style(color: Hsla) -> HighlightStyle {
    HighlightStyle {
        color: Some(color),
        font_style: Some(FontStyle::Italic),
        ..HighlightStyle::default()
    }
}

fn keyword_style(color: Hsla) -> HighlightStyle {
    HighlightStyle {
        color: Some(color),
        font_weight: Some(FontWeight::BOLD),
        ..HighlightStyle::default()
    }
}