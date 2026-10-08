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
                ("attribute".into(), color(0x9cdcfeff).into()),
                ("attribute.builtin".into(), color(0x9cdcfeff).into()),
                ("attribute.jsx".into(), color(0x9cdcfeff).into()),
                ("attribute.special".into(), color(0x9cdcfeff).into()),
                ("boolean".into(), color(0x569cd6ff).into()),
                ("charset".into(), color(0xce9178ff).into()),
                ("comment".into(), comment_style(color(0x6a9955ff))),
                ("comment.doc".into(), comment_style(color(0x6a9955ff))),
                ("comment.unused".into(), comment_style(color(0x6a9955ff))),
                ("concept".into(), color(0x4ec9b0ff).into()),
                ("constant".into(), color(0x4fc1ffff).into()),
                ("constant.builtin".into(), color(0x569cd6ff).into()),
                ("constructor".into(), color(0x4ec9b0ff).into()),
                ("decorator".into(), color(0xdcdcaaff).into()),
                ("diff.delta".into(), color(0x569cd6ff).into()),
                ("diff.delta.moved".into(), color(0xb5cea8ff).into()),
                ("diff.minus".into(), color(0xce9178ff).into()),
                ("diff.plus".into(), color(0xb5cea8ff).into()),
                ("embedded".into(), color(0xd6d6ddff).into()),
                ("emphasis".into(), italic_style(color(0xd6d6ddff))),
                ("emphasis.strong".into(), bold_style(color(0xd6d6ddff))),
                ("enum".into(), color(0x4ec9b0ff).into()),
                ("function".into(), color(0xdcdcaaff).into()),
                ("function.builtin".into(), color(0xdcdcaaff).into()),
                ("function.call".into(), color(0xdcdcaaff).into()),
                ("function.decorator".into(), color(0xdcdcaaff).into()),
                ("function.decorator.call".into(), color(0xdcdcaaff).into()),
                ("function.definition".into(), color(0xdcdcaaff).into()),
                ("function.kwargs".into(), color(0x9cdcfeff).into()),
                ("function.method".into(), color(0xdcdcaaff).into()),
                ("function.method.call".into(), color(0xdcdcaaff).into()),
                (
                    "function.method.constructor".into(),
                    color(0x4ec9b0ff).into(),
                ),
                ("function.special".into(), color(0xdcdcaaff).into()),
                (
                    "function.special.definition".into(),
                    color(0xdcdcaaff).into(),
                ),
                ("hint".into(), color(0x6d6d6dff).into()),
                ("import".into(), color(0xc586c0ff).into()),
                ("keyframes".into(), color(0xc586c0ff).into()),
                ("keyword".into(), color(0x569cd6ff).into()),
                ("keyword.control".into(), color(0xc586c0ff).into()),
                ("keyword.declaration".into(), color(0x569cd6ff).into()),
                ("keyword.definition".into(), color(0x569cd6ff).into()),
                ("keyword.directive".into(), color(0xc586c0ff).into()),
                ("keyword.function".into(), color(0x569cd6ff).into()),
                ("keyword.import".into(), color(0xc586c0ff).into()),
                ("keyword.jsdoc".into(), color(0x569cd6ff).into()),
                ("keyword.operator".into(), color(0xd6d6ddff).into()),
                ("keyword.operator.regex".into(), color(0xd16969ff).into()),
                ("keyword.preproc".into(), color(0xc586c0ff).into()),
                ("label".into(), color(0xc8c8c8ff).into()),
                ("label.regex".into(), color(0xce9178ff).into()),
                ("lifetime".into(), color(0x9cdcfeff).into()),
                ("link.text".into(), color(0x4ca8f5ff).into()),
                ("link.uri".into(), italic_style(color(0x4ca8f5ff))),
                ("link_text".into(), color(0x4ca8f5ff).into()),
                ("link_uri".into(), italic_style(color(0x4ca8f5ff))),
                ("markup.heading".into(), bold_style(color(0x569cd6ff))),
                ("markup.link.url".into(), color(0x4ca8f5ff).into()),
                ("media".into(), color(0x569cd6ff).into()),
                ("module".into(), color(0x4ec9b0ff).into()),
                ("namespace".into(), color(0x4ec9b0ff).into()),
                ("number".into(), color(0xb5cea8ff).into()),
                ("number.quantifier.regex".into(), color(0xd7ba7dff).into()),
                ("operator".into(), color(0xd6d6ddff).into()),
                ("predictive".into(), color(0x6d6d6dff).into()),
                ("preproc".into(), color(0xc586c0ff).into()),
                ("property".into(), color(0x9cdcfeff).into()),
                ("property.json_key".into(), color(0x9cdcfeff).into()),
                ("property.name".into(), color(0x9cdcfeff).into()),
                ("punctuation".into(), color(0x9aa0a6ff).into()),
                ("punctuation.bracket".into(), color(0xd6d6ddff).into()),
                ("punctuation.bracket.html".into(), color(0xd6d6ddff).into()),
                ("punctuation.bracket.jsx".into(), color(0xd6d6ddff).into()),
                ("punctuation.bracket.regex".into(), color(0xd6d6ddff).into()),
                ("punctuation.delimiter".into(), color(0x9aa0a6ff).into()),
                (
                    "punctuation.delimiter.html".into(),
                    color(0x9aa0a6ff).into(),
                ),
                ("punctuation.delimiter.jsx".into(), color(0x9aa0a6ff).into()),
                (
                    "punctuation.delimiter.regex".into(),
                    color(0x9aa0a6ff).into(),
                ),
                (
                    "punctuation.embedded.markup".into(),
                    color(0x569cd6ff).into(),
                ),
                ("punctuation.list_marker".into(), color(0x6796e6ff).into()),
                ("punctuation.markup".into(), color(0x569cd6ff).into()),
                ("punctuation.special".into(), color(0x569cd6ff).into()),
                ("selector.class".into(), color(0xd7ba7dff).into()),
                ("selector.id".into(), color(0xd7ba7dff).into()),
                ("selector.pseudo".into(), color(0xce9178ff).into()),
                ("storageclass".into(), color(0x569cd6ff).into()),
                ("string".into(), color(0xce9178ff).into()),
                ("string.doc".into(), color(0xce9178ff).into()),
                ("string.escape".into(), color(0xd7ba7dff).into()),
                ("string.escape.regex".into(), color(0xd7ba7dff).into()),
                ("string.regex".into(), color(0xd16969ff).into()),
                ("string.special".into(), color(0xce9178ff).into()),
                ("string.special.path".into(), color(0xce9178ff).into()),
                ("string.special.symbol".into(), color(0x4fc1ffff).into()),
                ("supports".into(), color(0x4ec9b0ff).into()),
                ("tag".into(), color(0x569cd6ff).into()),
                ("tag.component.jsx".into(), color(0x569cd6ff).into()),
                ("tag.doctype".into(), color(0xce9178ff).into()),
                ("tag.jsx".into(), color(0x569cd6ff).into()),
                ("text".into(), color(0xd6d6ddff).into()),
                ("text.jsx".into(), color(0xd6d6ddff).into()),
                ("text.literal".into(), color(0xce9178ff).into()),
                ("text.literal.markup".into(), color(0xce9178ff).into()),
                ("title".into(), bold_style(color(0x569cd6ff))),
                ("type".into(), color(0x4ec9b0ff).into()),
                ("type.builtin".into(), color(0x4ec9b0ff).into()),
                ("type.class".into(), color(0x4ec9b0ff).into()),
                ("type.class.builtin".into(), color(0x4ec9b0ff).into()),
                ("type.class.call".into(), color(0x4ec9b0ff).into()),
                ("type.class.definition".into(), color(0x4ec9b0ff).into()),
                ("type.class.inheritance".into(), color(0x4ec9b0ff).into()),
                ("type.definition".into(), color(0x4ec9b0ff).into()),
                ("type.interface".into(), color(0x4ec9b0ff).into()),
                ("type.jsdoc".into(), color(0x4ec9b0ff).into()),
                ("type.name".into(), color(0x4ec9b0ff).into()),
                ("type.qualifier".into(), color(0x4ec9b0ff).into()),
                ("type.unit".into(), color(0xb5cea8ff).into()),
                ("variable".into(), color(0x9cdcfeff).into()),
                ("variable.builtin".into(), color(0x569cd6ff).into()),
                ("variable.jsdoc".into(), color(0x9cdcfeff).into()),
                ("variable.other.member".into(), color(0x9cdcfeff).into()),
                ("variable.parameter".into(), italic_style(color(0x9cdcfeff))),
                ("variable.special".into(), color(0xd7ba7dff).into()),
                ("variant".into(), color(0x4ec9b0ff).into()),
                ("warning".into(), color(0xe5b95cff).into()),
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

fn italic_style(color: Hsla) -> HighlightStyle {
    HighlightStyle {
        color: Some(color),
        font_style: Some(FontStyle::Italic),
        ..HighlightStyle::default()
    }
}

fn bold_style(color: Hsla) -> HighlightStyle {
    HighlightStyle {
        color: Some(color),
        font_weight: Some(FontWeight::BOLD),
        ..HighlightStyle::default()
    }
}
