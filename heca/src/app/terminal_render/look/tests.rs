use super::*;

/// **A pane re-tints what it holds; it does not redefine what a border is.**
///
/// Verified by sabotage: writing the frame colour into `border` here makes this fail, which is
/// the state that shipped the hover-border difference between an active pane and every other.
#[test]
fn a_panes_frame_does_not_become_the_border_every_control_inside_it_reads() {
    let chrome = GuiTheme::default();
    let themed = pane_gui_theme(chrome.clone(), GuiColor::rgb(1, 2, 3));

    assert_eq!(
        themed.colors.accent, chrome.colors.accent,
        "the pane's own hue is published on its shell, not swapped into the theme here"
    );
    assert_eq!(
        themed.colors.border, chrome.colors.border,
        "…and the border token every control reads is the chrome's, whatever this pane's frame is"
    );
    assert_eq!(
        themed.colors.border_width, chrome.colors.border_width,
        "…and so is its width"
    );
    assert_eq!(
        themed.colors.border_radius, chrome.colors.border_radius,
        "…and its radius"
    );
}
