use fluid_core::settings::{AppSettings, Orientation, TempUnit};
use iced::widget::{button, column, container, mouse_area, pick_list, row, scrollable, slider, text, text_input, toggler, Space};
use iced::{Border, Element, Length};
use crate::style::Palette;
use crate::Message;

const TILES: [&str; 6] = ["Clock","CPU","GPU","RAM","Network","Storage"];
const TILE_INTERNAL: [&str; 6] = ["Clock","CPU","GPU","RAM","Network","Disk"];

const FONT_DEFAULT: &str = "(Default)";

pub fn view<'a>(
    settings: &AppSettings, p: Palette, win_id: iced::window::Id,
    theme_name: String, disks: Vec<String>, adapters: Vec<String>,
    fonts: Vec<String>,
    editing_color: Option<u8>,
) -> Element<'a, Message> {
    // ── Style helpers ──
    let sh = |label: &str| -> Element<'a, Message> {
        row![
            text(label.to_string()).size(13)
                .font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.accent) }),
            Space::with_width(5),
            qmark(p),
        ].align_y(iced::Alignment::Center).into()
    };
    let fl = |t: &str| -> Element<'a, Message> {
        text(t.to_string()).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
            .into()
    };
    let vl = |t: String| -> Element<'a, Message> {
        text(t).size(11)
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
            .into()
    };
    let pill = |label_text: String, active: bool, msg: Message| -> Element<'a, Message> {
        button(text(label_text).size(11).font(iced::Font::with_name("Segoe UI Symbol")))
            .padding([4, 14])
            .style(move |_: &iced::Theme, _: button::Status| button::Style {
                background: Some(iced::Background::Color(if active { p.accent } else { p.tile })),
                text_color: if active { iced::Color::WHITE } else { p.text },
                border: Border { radius: 4.0.into(), ..Border::default() },
                ..Default::default()
            })
            .on_press(msg).into()
    };
    let cycle_btn = |label_text: String, msg: Message| -> Element<'a, Message> {
        button(
            container(text(label_text).size(11)
                .style(move |_| iced::widget::text::Style { color: Some(p.text) })
            ).center_x(Length::Fill)
        )
        .width(Length::Fill)
        .padding([5, 8])
        .style(move |_: &iced::Theme, _: button::Status| button::Style {
            background: Some(iced::Background::Color(p.tile)),
            text_color: p.text,
            border: Border { radius: 4.0.into(), width: 1.0, color: p.muted },
            ..Default::default()
        })
        .on_press(msg).into()
    };
    // Paired slider: label + value on one row, slider below, in half-width
    let pslider = |label_text: &str, value_text: String, min: f32, max: f32, val: f32, step: f32, msg: fn(f32)->Message| -> Element<'a, Message> {
        column![
            row![fl(label_text), Space::with_width(Length::Fill), vl(value_text)],
            slider(min..=max, val, msg).step(step),
        ].spacing(2).width(Length::FillPortion(1)).into()
    };

    // ════════════════════════════════════════════════════════════
    //  LEFT COLUMN  (matches C# SettingsWindow.xaml left panel)
    // ════════════════════════════════════════════════════════════

    // ── Tiles: 3x2 toggle grid ──
    let mut t_r0 = Vec::<Element<'a, Message>>::new();
    let mut t_r1 = Vec::<Element<'a, Message>>::new();
    for (i, (display, internal)) in TILES.iter().zip(TILE_INTERNAL.iter()).enumerate() {
        let visible = settings.visible_tiles.iter().any(|v| v == internal);
        let name = internal.to_string();
        let t: Element<'a, Message> = row![
            toggler(visible).size(14).on_toggle(move |on| Message::ToggleTile(name.clone(), on)),
            text(display.to_string()).size(11)
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(6).align_y(iced::Alignment::Center).width(Length::FillPortion(1)).into();
        if i < 3 { t_r0.push(t); } else { t_r1.push(t); }
    }
    let tiles_grid = column![row(t_r0).spacing(4), row(t_r1).spacing(4)].spacing(6);

    let fahrenheit = settings.temperature_unit == TempUnit::Fahrenheit;
    let temp_row: Element<'a, Message> = row![
        qmark(p),
        Space::with_width(4),
        text("CPU temperature").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        Space::with_width(6),
        text("Active").size(11).style(move |_| iced::widget::text::Style { color: Some(iced::Color::from_rgb(0.2, 0.8, 0.4)) }),
        Space::with_width(8),
        pill("\u{00B0}C".into(), !fahrenheit, Message::SetFahrenheit(false)),
        pill("\u{00B0}F".into(), fahrenheit, Message::SetFahrenheit(true)),
    ].align_y(iced::Alignment::Center).spacing(0).into();

    // ── Tile Labels: CPU/GPU with Auto/Custom pills ──
    let cpu_auto = settings.cpu_custom_name.is_empty();
    let gpu_auto = settings.gpu_custom_name.is_empty();
    let tile_labels = column![
        row![
            text("CPU").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }).width(28),
            text_input("auto", &settings.cpu_custom_name).size(11).on_input(Message::SetCpuName).width(Length::Fill),
            Space::with_width(8),
            pill("Auto".into(), cpu_auto, Message::SetCpuName(String::new())),
            pill("Custom".into(), !cpu_auto, Message::SetCpuName("Custom".into())),
        ].spacing(4).align_y(iced::Alignment::Center),
        row![
            text("GPU").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }).width(28),
            text_input("auto", &settings.gpu_custom_name).size(11).on_input(Message::SetGpuName).width(Length::Fill),
            Space::with_width(8),
            pill("Auto".into(), gpu_auto, Message::SetGpuName(String::new())),
            pill("Custom".into(), !gpu_auto, Message::SetGpuName("Custom".into())),
        ].spacing(4).align_y(iced::Alignment::Center),
    ].spacing(6);

    // ── Layout ──
    let layout_pills = row![
        pill("Horizontal".into(), settings.orientation == Orientation::Horizontal, Message::SetOrientation(Orientation::Horizontal)),
        pill("Vertical".into(), settings.orientation == Orientation::Vertical, Message::SetOrientation(Orientation::Vertical)),
    ].spacing(4);

    // ── Behavior: togglers in pairs + hotkey + paired sliders ──
    let sw = |label_text: &str, on: bool, msg: fn(bool)->Message| -> Element<'a, Message> {
        row![
            toggler(on).size(14).on_toggle(msg),
            text(label_text.to_string()).size(11)
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(6).align_y(iced::Alignment::Center).width(Length::FillPortion(1)).into()
    };

    let behavior = column![
        row![sw("Always on top", settings.always_on_top, Message::SetAlwaysOnTop), sw("Click-through", settings.click_through, Message::SetClickThrough)].spacing(8),
        row![sw("Snap to edges", settings.snap_to_edges, Message::SetSnap), sw("Snap to windows", settings.snap_to_windows, Message::SetSnapWindows)].spacing(8),
        sw("Run at Windows startup", settings.run_at_startup, Message::SetRunAtStartup),
        Space::with_height(4),
        fl("Click-through hotkey"),
        row![
            text_input("", &settings.click_through_hotkey).size(11).width(150),
            text("click to set").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            button(text("\u{2715}").size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
                .padding([2, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() })
                .on_press(Message::SetHotkey(String::new())),
        ].spacing(6).align_y(iced::Alignment::Center),
        Space::with_height(4),
        // Paired sliders: Opacity + Update interval
        row![
            pslider("Opacity", format!("{:.0}%", settings.widget_opacity * 100.0), 0.3, 1.0, settings.widget_opacity, 0.05, Message::SetOpacity),
            Space::with_width(8),
            pslider("Update interval", format!("{} ms", settings.update_interval_ms), 250.0, 5000.0, settings.update_interval_ms as f32, 250.0, Message::SetInterval),
        ],
        // UI scale + Tile width
        row![
            pslider("UI scale", format!("{:.2}x", settings.ui_scale), 0.75, 1.5, settings.ui_scale, 0.05, Message::SetUiScale),
            Space::with_width(8),
            pslider("Tile width", format!("{:.0}px", settings.tile_width), 110.0, 200.0, settings.tile_width, 5.0, Message::SetTileWidth),
        ],
        // Tile height alone
        column![
            row![fl("Tile height"), Space::with_width(Length::Fill), vl(format!("{:.0}px", settings.tile_height))],
            slider(80.0..=150.0, settings.tile_height, Message::SetTileHeight).step(2.0),
        ].spacing(2),
    ].spacing(4);

    // ── Network: paired grid ──
    let traffic_label = format!("\u{2193} {} \u{2191}", settings.network_traffic_indicator);
    let adapter_value = if settings.network_adapter_name.is_empty() { "All adapters".to_string() } else { settings.network_adapter_name.clone() };
    let selected_adapter = if adapters.contains(&adapter_value) { Some(adapter_value) } else { Some("All adapters".to_string()) };
    let network = column![
        row![
            column![fl("Traffic indicator"), cycle_btn(traffic_label, Message::TrafficCycle)].width(Length::FillPortion(1)).spacing(2),
            Space::with_width(12),
            pslider("Arrow spacing", format!("{:.0}px", settings.network_arrow_spacing), 8.0, 40.0, settings.network_arrow_spacing, 1.0, Message::SetArrowSpacing),
        ],
        Space::with_height(4),
        row![
            column![fl("Monitor adapter"), pick_list(adapters, selected_adapter, Message::SetAdapter).text_size(11).width(Length::Fill)].width(Length::FillPortion(1)).spacing(2),
            Space::with_width(12),
            pslider("Arrow size", format!("{:+}pt", settings.arrow_font_offset), -5.0, 10.0, settings.arrow_font_offset as f32, 1.0, Message::SetArrowFontOffset),
        ],
    ].spacing(2);

    // ── Disk: paired grid ──
    let disk_label_text = format!("Show: {}", settings.disk_label_style);
    let selected_disk = if disks.contains(&settings.selected_disk_mount) { Some(settings.selected_disk_mount.clone()) } else { disks.first().cloned() };
    let disk = column![
        row![
            column![fl("Tile label"), cycle_btn(disk_label_text, Message::DiskLabelCycle)].width(Length::FillPortion(1)).spacing(2),
            Space::with_width(12),
            pslider("R: / W: spacing", format!("{:.0}px", settings.disk_label_spacing), 0.0, 40.0, settings.disk_label_spacing, 1.0, Message::SetDiskLabelSpacing),
        ],
        Space::with_height(4),
        row![
            column![fl("Monitor disk"), pick_list(disks, selected_disk, Message::SetDisk).text_size(11).width(Length::Fill)].width(Length::FillPortion(1)).spacing(2),
            Space::with_width(12),
            pslider("R: / W: size", format!("{:+}pt", settings.disk_label_font_offset), -5.0, 10.0, settings.disk_label_font_offset as f32, 1.0, Message::SetDiskLabelFontOffset),
        ],
    ].spacing(2);

    let left_col = column![
        sh("Tiles"), tiles_grid, temp_row,
        Space::with_height(4),
        sh("Tile Labels"), tile_labels,
        Space::with_height(4),
        sh("Layout"), layout_pills,
        Space::with_height(4),
        sh("Behavior"), behavior,
        Space::with_height(4),
        sh("Network"), network,
        Space::with_height(4),
        sh("Disk"), disk,
    ].spacing(3).width(Length::Fixed(300.0));

    // ════════════════════════════════════════════════════════════
    //  RIGHT COLUMN  (Appearance / Font / Remote / Updates)
    // ════════════════════════════════════════════════════════════

    // ── Saved Themes row ──
    let mut saved_row = row![
        text("Saved Themes").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        Space::with_width(8),
    ].spacing(0).align_y(iced::Alignment::Center);
    for i in 1..=5u8 {
        saved_row = saved_row.push(
            button(text(i.to_string()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }))
                .padding([3, 8])
                .style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), width: 1.0, color: p.muted }, ..Default::default() })
                .on_press(Message::PresetSlotClick(i - 1))
        );
        saved_row = saved_row.push(Space::with_width(3));
    }
    saved_row = saved_row.push(Space::with_width(6));
    saved_row = saved_row.push(
        button(text("\u{2199}").size(12).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
            .padding([3, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), ..Border::default() }, ..Default::default() })
            .on_press(Message::Noop)
    );
    saved_row = saved_row.push(Space::with_width(3));
    saved_row = saved_row.push(
        button(text("\u{2197}").size(12).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
            .padding([3, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), ..Border::default() }, ..Default::default() })
            .on_press(Message::Noop)
    );

    // ── Preset Themes cycler ──
    let preset_cycler = row![
        button(text("\u{2193}").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
            .padding([3, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), ..Border::default() }, ..Default::default() }).on_press(Message::Noop),
        Space::with_width(3),
        button(text("\u{1F3B2}").size(12).font(iced::Font::with_name("Segoe UI Symbol")))
            .padding([3, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), ..Border::default() }, ..Default::default() }).on_press(Message::ThemeDice),
        Space::with_width(3),
        pill("\u{2039}".into(), false, Message::ThemePrev),
        button(
            container(row![
                container(Space::new(7, 7)).style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(p.accent)), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() }),
                Space::with_width(5),
                text(theme_name).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            ].align_y(iced::Alignment::Center)).center_x(Length::Fill)
        ).width(Length::Fill).padding([4, 6])
        .style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() })
        .on_press(Message::ThemeNext),
        pill("\u{203A}".into(), false, Message::ThemeNext),
        Space::with_width(3),
        button(text("\u{1F4CB}").size(11).font(iced::Font::with_name("Segoe UI Symbol")))
            .padding([3, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), ..Border::default() }, ..Default::default() }).on_press(Message::Noop),
    ].align_y(iced::Alignment::Center).spacing(2);

    // ── Skins box ──
    let skins_box = container(column![
        text("Skins").size(9).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        row![
            button(text("\u{21B6}").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
                .padding([3, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), ..Border::default() }, ..Default::default() }).on_press(Message::SkinPrev),
            button(text("\u{1F3B2}").size(11).font(iced::Font::with_name("Segoe UI Symbol")))
                .padding([3, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), ..Border::default() }, ..Default::default() }).on_press(Message::SkinDice),
            Space::with_width(4),
            pill("\u{2039}".into(), false, Message::SkinPrev),
            container(
                row![
                    container(Space::new(2, 14)).style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(p.accent)), ..Default::default() }),
                    Space::with_width(6),
                    text(settings.active_skin.clone()).size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
                ].align_y(iced::Alignment::Center)
            ).width(Length::Fill).center_x(Length::Fill).padding([4, 8])
            .style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() }),
            pill("\u{203A}".into(), false, Message::SkinNext),
            button(text("\u{1F4CB}").size(11).font(iced::Font::with_name("Segoe UI Symbol")))
                .padding([3, 6]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 3.0.into(), ..Border::default() }, ..Default::default() }).on_press(Message::Noop),
        ].align_y(iced::Alignment::Center).spacing(3),
        Space::with_height(4),
        text("Colors").size(9).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        preset_cycler,
    ].spacing(3))
    .padding(8)
    .style(move |_| iced::widget::container::Style {
        border: Border { radius: 6.0.into(), width: 1.0, color: iced::Color { a: 0.3, ..p.muted } },
        ..Default::default()
    });

    // ── 5 big swatches with labels + hex ──
    let swatch_data: [(u8, &str, &str); 5] = [
        (0, "Background", &settings.theme_bg),
        (1, "Tile", &settings.theme_tile),
        (2, "Accent", &settings.theme_accent),
        (3, "Text", &settings.theme_text),
        (4, "Muted", &settings.theme_muted),
    ];
    let mut swatch_cols: Vec<Element<'a, Message>> = Vec::new();
    for (slot, name, hex) in swatch_data {
        let c = crate::style::swatch_color(hex);
        let hex_s = hex.to_string();
        let is_accent = slot == 2;
        let short_hex = if hex_s.len() > 4 { format!("#{}", &hex_s[3..]) } else { hex_s.clone() };
        let col: Element<'a, Message> = column![
            button(Space::new(Length::Fill, 36))
                .padding(0)
                .style(move |_, _| button::Style {
                    background: Some(iced::Background::Color(c)),
                    border: Border { radius: 6.0.into(), width: if is_accent { 2.0 } else { 0.0 }, color: p.text },
                    ..Default::default()
                })
                .on_press(Message::EditColor(slot)),
            text(name.to_string()).size(9)
                .font(if is_accent { iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT } } else { iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(if is_accent { p.text } else { p.muted }) }),
            text(short_hex).size(9)
                .style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        ].spacing(2).align_x(iced::Alignment::Center).width(Length::FillPortion(1)).into();
        swatch_cols.push(col);
    }
    let swatch_strip = row(swatch_cols).spacing(6);

    // Inline hex editor for the swatch the user clicked (EditColor toggles it).
    let color_editor: Element<'a, Message> = if let Some(slot) = editing_color {
        let (lbl, hex) = match slot {
            0 => ("Background", settings.theme_bg.clone()),
            1 => ("Tile", settings.theme_tile.clone()),
            2 => ("Accent", settings.theme_accent.clone()),
            3 => ("Text", settings.theme_text.clone()),
            _ => ("Muted", settings.theme_muted.clone()),
        };
        row![
            text(format!("{} hex", lbl)).size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            Space::with_width(8),
            text_input("#AARRGGBB", &hex).size(11).font(iced::Font::with_name("Consolas")).width(160)
                .on_input(move |s| Message::SetHexColor(slot, s)),
            Space::with_width(8),
            button(text("done").size(10).style(move |_| iced::widget::text::Style { color: Some(p.muted) }))
                .padding([3, 10]).style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 4.0.into(), ..Border::default() }, ..Default::default() })
                .on_press(Message::EditColor(slot)),
        ].spacing(0).align_y(iced::Alignment::Center).into()
    } else {
        Space::with_height(0).into()
    };

    let appearance = column![
        saved_row,
        Space::with_height(4),
        fl("Preset Themes"),
        skins_box,
        Space::with_height(6),
        swatch_strip,
        Space::with_height(4),
        color_editor,
        row![fl("Muted text visibility"), Space::with_width(Length::Fill), vl(format!("{:.2}", settings.muted_contrast))],
        slider(0.5..=2.0, settings.muted_contrast, Message::SetMutedContrast).step(0.05),
    ].spacing(3);

    // ── Font: sync toggle + font pickers + 3-col size sliders ──
    let fonts = column![
        row![
            toggler(settings.sync_fonts).size(14).on_toggle(Message::SetSyncFonts),
            text("Sync fonts").size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            Space::with_width(16),
            toggler(settings.randomize_fonts_on_dice).size(14).on_toggle(Message::SetRandomizeFonts),
            text("Allow random fonts with \u{1F3B2} button").size(11)
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ].spacing(6).align_y(iced::Alignment::Center),
        {
            let mut opts = vec![FONT_DEFAULT.to_string()];
            opts.extend(fonts.iter().cloned());
            let font_picker = |slot: u8, current: &Option<String>| -> Element<'a, Message> {
                let mut o = opts.clone();
                let sel = match current {
                    Some(f) if !f.is_empty() => {
                        if !o.contains(f) { o.push(f.clone()); }
                        f.clone()
                    }
                    _ => FONT_DEFAULT.to_string(),
                };
                pick_list(o, Some(sel), move |s: String| {
                    let name = if s == FONT_DEFAULT { String::new() } else { s };
                    Message::SetFont(slot, name)
                }).text_size(11).width(Length::Fill).into()
            };
            row![
                column![fl("Primary font"), font_picker(0, &settings.primary_font)].width(Length::FillPortion(1)).spacing(2),
                column![fl("Secondary font"), font_picker(1, &settings.secondary_font)].width(Length::FillPortion(1)).spacing(2),
                column![fl("Indicator font"), font_picker(2, &settings.indicator_font)].width(Length::FillPortion(1)).spacing(2),
            ].spacing(6)
        },
        fl("Font sizes"),
        row![
            column![
                fl("Primary"),
                slider(-4.0..=8.0, settings.primary_font_offset as f32, Message::SetPrimaryFontOffset).step(1.0),
                vl(format!("{:+}pt", settings.primary_font_offset)),
            ].width(Length::FillPortion(1)).spacing(2).align_x(iced::Alignment::Center),
            column![
                fl("Secondary"),
                slider(-4.0..=8.0, settings.secondary_font_offset as f32, Message::SetSecondaryFontOffset).step(1.0),
                vl(format!("{:+}pt", settings.secondary_font_offset)),
            ].width(Length::FillPortion(1)).spacing(2).align_x(iced::Alignment::Center),
            column![
                fl("Indicators"),
                slider(-4.0..=8.0, settings.indicator_font_offset as f32, Message::SetIndicatorFontOffset).step(1.0),
                vl(format!("{:+}pt", settings.indicator_font_offset)),
            ].width(Length::FillPortion(1)).spacing(2).align_x(iced::Alignment::Center),
        ].spacing(8),
    ].spacing(4);

    // ── Remote Monitoring ──
    let remote = row![
        sh("Remote Monitoring"),
        Space::with_width(8),
        toggler(settings.remote_enabled).size(14).on_toggle(Message::SetRemoteEnabled),
    ].align_y(iced::Alignment::Center);

    // ── Updates box ──
    let updates = container(column![
        row![
            text("Current version").size(11).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
            Space::with_width(Length::Fill),
            text("v2.0.0-alpha").size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }),
        ],
        Space::with_height(4),
        row![
            pill("Auto".into(), settings.update_check_mode == fluid_core::settings::UpdateMode::Auto, Message::SetUpdateMode("Auto".into())),
            pill("Manual".into(), settings.update_check_mode == fluid_core::settings::UpdateMode::Manual, Message::SetUpdateMode("Manual".into())),
            pill("Off".into(), settings.update_check_mode == fluid_core::settings::UpdateMode::Off, Message::SetUpdateMode("Off".into())),
            Space::with_width(Length::Fill),
            button(text("Check now").size(11).style(move |_| iced::widget::text::Style { color: Some(p.text) }))
                .padding([4, 12])
                .style(move |_,_| button::Style { background: Some(iced::Background::Color(p.tile)), border: Border { radius: 4.0.into(), width: 1.0, color: p.muted }, ..Default::default() })
                .on_press(Message::Noop),
        ].spacing(4).align_y(iced::Alignment::Center),
        row![
            Space::with_width(Length::Fill),
            text("Last checked: never").size(9).style(move |_| iced::widget::text::Style { color: Some(p.muted) }),
        ],
    ].spacing(3))
    .padding([8, 12])
    .width(Length::Fill)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(p.tile)),
        border: Border { radius: 6.0.into(), ..Border::default() },
        ..Default::default()
    });

    let right_col = column![
        sh("Appearance"), appearance,
        Space::with_height(6),
        sh("Font"), fonts,
        Space::with_height(6),
        remote,
        Space::with_height(6),
        sh("Updates"), updates,
    ].spacing(3).width(Length::Fill);

    // ════════════════════════════════════════════
    //  ASSEMBLY
    // ════════════════════════════════════════════

    let columns = row![left_col, Space::with_width(20), right_col];

    // 32px caption: "Settings" left, ✕ right, whole bar draggable
    let close_btn = button(
        text("\u{2715}").size(16).font(iced::Font::with_name("Segoe UI Symbol"))
            .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    ).padding([2, 8]).style(|_,_| button::Style { background: None, ..Default::default() }).on_press(Message::SaveClose);

    let caption = mouse_area(
        container(row![
            text("Settings").size(13).font(iced::Font { weight: iced::font::Weight::Semibold, ..iced::Font::DEFAULT })
                .style(move |_| iced::widget::text::Style { color: Some(p.text) }),
            Space::with_width(Length::Fill),
            close_btn,
        ].align_y(iced::Alignment::Center)).width(Length::Fill).height(32).padding([0, 6])
    ).on_press(Message::DragWindow(win_id));

    // Bottom bar: [?|⚙] split + Reset + Save
    let split_left = button(text("?").size(14).font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
        .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    ).padding([4, 12]).style(move |_,_| button::Style {
        background: Some(iced::Background::Color(p.tile)),
        border: Border { radius: iced::border::Radius { top_left: 7.0, top_right: 0.0, bottom_right: 0.0, bottom_left: 7.0 }, ..Border::default() },
        ..Default::default()
    }).on_press(Message::OpenHelp);
    let split_divider = container(Space::new(1, 24)).style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(iced::Color { a: 0.4, ..p.muted })), ..Default::default() });
    let split_right = button(text("\u{2699}").size(13).font(iced::Font::with_name("Segoe UI Symbol"))
        .style(move |_| iced::widget::text::Style { color: Some(p.muted) })
    ).padding([4, 12]).style(move |_,_| button::Style {
        background: Some(iced::Background::Color(p.tile)),
        border: Border { radius: iced::border::Radius { top_left: 0.0, top_right: 7.0, bottom_right: 7.0, bottom_left: 0.0 }, ..Border::default() },
        ..Default::default()
    }).on_press(Message::OpenTools);

    let reset_btn = button(text("Reset to Defaults").size(11)
        .style(move |_| iced::widget::text::Style { color: Some(iced::Color::from_rgb(0.9, 0.3, 0.3)) })
    ).padding([6, 14]).style(move |_,_| button::Style {
        background: None,
        border: Border { radius: 6.0.into(), width: 1.0, color: iced::Color::from_rgb(0.9, 0.3, 0.3) },
        ..Default::default()
    }).on_press(Message::ResetDefaults);

    let save_btn = button(text("Save and Close").size(11)
        .style(move |_| iced::widget::text::Style { color: Some(iced::Color::WHITE) })
    ).padding([6, 20]).style(move |_,_| button::Style {
        background: Some(iced::Background::Color(p.accent)),
        border: Border { radius: 6.0.into(), ..Border::default() },
        ..Default::default()
    }).on_press(Message::SaveClose);

    let divider = container(Space::new(Length::Fill, 1)).style(move |_| iced::widget::container::Style { background: Some(iced::Background::Color(iced::Color { a: 0.3, ..p.muted })), ..Default::default() });

    let bottom_bar = container(
        row![split_left, split_divider, split_right, Space::with_width(8), reset_btn, Space::with_width(Length::Fill), save_btn]
            .align_y(iced::Alignment::Center)
    ).width(Length::Fill).padding(iced::Padding { top: 10.0, right: 0.0, bottom: 0.0, left: 0.0 });

    let content = column![
        caption,
        scrollable(container(columns).padding(iced::Padding { top: 4.0, right: 6.0, bottom: 8.0, left: 0.0 })).height(Length::Fill),
        divider,
        bottom_bar,
    ];

    container(content)
        .width(Length::Fill).height(Length::Fill)
        .padding(iced::Padding { top: 0.0, right: 20.0, bottom: 10.0, left: 20.0 })
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(p.bg)),
            ..Default::default()
        })
        .into()
}

fn qmark<'a>(p: Palette) -> Element<'a, Message> {
    container(
        text("?").size(9).font(iced::Font { weight: iced::font::Weight::Bold, ..iced::Font::DEFAULT })
            .style(move |_| iced::widget::text::Style { color: Some(iced::Color::WHITE) })
    )
    .width(14).height(14)
    .center_x(14).center_y(14)
    .style(move |_| iced::widget::container::Style {
        background: Some(iced::Background::Color(iced::Color { a: 0.4, ..p.muted })),
        border: Border { radius: 7.0.into(), ..Border::default() },
        ..Default::default()
    })
    .into()
}



