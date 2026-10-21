use super::*;

#[test]
fn new_content_follows_only_while_the_reader_is_at_the_bottom() {
    let mut region = region_with_children(&[300.0]).follow_bottom(true);
    region.on_layout();
    assert_eq!(region.scroll_offset.get_untracked(), 200.0);
    region.base.children[0].base_mut().bounds.size.h = 400.0;
    region.base.children[0].base_mut().bounds.loc.y = 0.0;
    region.on_layout();
    assert_eq!(region.scroll_offset.get_untracked(), 300.0);
    region.scroll_to(50.0);
    region.base.children[0].base_mut().bounds.size.h = 500.0;
    region.base.children[0].base_mut().bounds.loc.y = 0.0;
    region.on_layout();
    assert_eq!(region.scroll_offset.get_untracked(), 50.0);
    region.scroll_to(400.0);
    region.base.children[0].base_mut().bounds.size.h = 600.0;
    region.base.children[0].base_mut().bounds.loc.y = 0.0;
    region.on_layout();
    assert_eq!(region.scroll_offset.get_untracked(), 500.0);
}
