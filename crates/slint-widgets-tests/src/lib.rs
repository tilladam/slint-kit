//! Unpublished test harness; production components have no Rust runtime dependencies.

#[cfg(test)]
mod tests {
    use i_slint_backend_testing::{self as backend, ElementHandle};
    use slint::{ComponentHandle, Model, ModelRc, VecModel};
    use std::rc::Rc;
    slint::include_modules!();

    fn press(component: &impl ComponentHandle, keys: [slint::platform::Key; 1]) {
        let text: slint::SharedString = char::from(keys[0]).into();
        component
            .window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: text.clone() });
        component
            .window()
            .dispatch_event(slint::platform::WindowEvent::KeyReleased { text });
    }

    #[test]
    fn buttons_and_palette_work_without_a_display() {
        backend::init_no_event_loop();
        let buttons = Buttons::new().unwrap();
        buttons.show().unwrap();
        let copy = ElementHandle::find_by_accessible_label(&buttons, "Copy")
            .next()
            .unwrap();
        copy.invoke_accessible_default_action();
        assert_eq!(buttons.get_copies(), 1);
        buttons.invoke_focus_copy();
        press(&buttons, [slint::platform::Key::Return]);
        assert_eq!(buttons.get_copies(), 2);
        press(&buttons, [slint::platform::Key::Tab]);
        press(&buttons, [slint::platform::Key::Space]);
        assert!(buttons.get_armed());
        buttons.invoke_focus_copy();
        assert!(!buttons.get_armed());
        buttons.invoke_focus_confirm();
        press(&buttons, [slint::platform::Key::Return]);
        backend::mock_elapsed_time(std::time::Duration::from_millis(2600));
        slint::platform::update_timers_and_animations();
        assert!(!buttons.get_armed());
        let confirm = ElementHandle::find_by_accessible_label(&buttons, "delete")
            .next()
            .unwrap();
        confirm.invoke_accessible_default_action();
        assert!(buttons.get_armed());
        confirm.invoke_accessible_default_action();
        assert_eq!(buttons.get_confirmations(), 1);
        copy.mock_single_click(slint::platform::PointerEventButton::Left);
        assert_eq!(buttons.get_copies(), 3);

        let palette = PaletteTest::new().unwrap();
        let rows = Rc::new(VecModel::from(
            (0..60)
                .map(|i| PaletteRow {
                    id: i.to_string().into(),
                    label: format!("row {i}").into(),
                    ..Default::default()
                })
                .collect::<Vec<_>>(),
        ));
        palette.set_entries(ModelRc::from(rows.clone()));
        palette.show().unwrap();
        for _ in 0..40 {
            press(&palette, [slint::platform::Key::DownArrow]);
        }
        press(&palette, [slint::platform::Key::Return]);
        assert_eq!(palette.get_executed(), "40");
        let selected = ElementHandle::find_by_accessible_label(&palette, "row 40")
            .next()
            .unwrap();
        let y = selected.absolute_position().y;
        assert!(
            y >= 0.0 && y + selected.size().height <= 480.0,
            "selected row at {y}"
        );
        while rows.row_count() > 1 {
            rows.remove(rows.row_count() - 1);
        }
        press(&palette, [slint::platform::Key::Return]);
        assert_eq!(palette.get_executed(), "0");
        rows.remove(0);
        palette.set_executed("none".into());
        press(&palette, [slint::platform::Key::Return]);
        assert_eq!(palette.get_executed(), "none");
        palette.set_entries(ModelRc::new(VecModel::from(vec![PaletteRow {
            id: "new".into(),
            ..Default::default()
        }])));
        press(&palette, [slint::platform::Key::Return]);
        assert_eq!(palette.get_executed(), "new");
        press(&palette, [slint::platform::Key::Escape]);
        assert_eq!(palette.get_closes(), 1);

        // The read-only model must forward notifications to a real repeater.
        let view = ModelTest::new().unwrap();
        let mut keyed = slint_model_sync::KeyedModel::<&str, slint::SharedString>::new();
        view.set_rows(keyed.model_rc());
        view.show().unwrap();
        keyed.apply(&[("a", 0), ("b", 0)], |key| (*key).into());
        assert_eq!(
            ElementHandle::find_by_accessible_label(&view, "a").count(),
            1
        );
        keyed.apply(&[("b", 1), ("a", 0), ("c", 0)], |key| {
            format!("{key} updated").into()
        });
        assert_eq!(
            ElementHandle::find_by_accessible_label(&view, "b updated").count(),
            1
        );
        assert_eq!(
            ElementHandle::find_by_accessible_label(&view, "c updated").count(),
            1
        );
        keyed.apply(&[("a", 0)], |_| panic!("retained row rebuilt"));
        assert_eq!(
            ElementHandle::find_by_accessible_label(&view, "b updated").count(),
            0
        );
    }

    #[test]
    fn slint_renders_segment_reference_links() {
        let source = "[docs][d]\n\n[d]: https://example.org";
        let md_segments::segmenter::Segment::Prose(md) =
            &md_segments::segmenter::segment_markdown(source)[0]
        else {
            panic!()
        };
        let reference = slint::StyledText::from_markdown(md).unwrap();
        let inline = slint::StyledText::from_markdown("[docs](https://example.org)").unwrap();
        assert_eq!(reference, inline);
    }
}
