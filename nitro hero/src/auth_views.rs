use crate::components::rgb;
use crate::{
    Preview,
    components::{
        self, ACCENT, MATERIAL_DARK, MATERIAL_GREEN, MATERIAL_PANEL, MATERIAL_RED, MATERIAL_SUBTLE,
        StrokeKind, badge, label, material_surface,
    },
    state::{
        AppTheme, AuthTab, FriendRequest, SettingsTab, ShellView, initials_for,
        is_valid_username,
    },
};
use gpui::{
    Context, Div, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString,
    Stateful, StatefulInteractiveElement, Styled, Window, div, prelude::*, px, rgba, svg,
};
use gpui_component::input::Input;

fn material_interactive(
    id: impl Into<gpui::ElementId>,
    fill_hex: u32,
    radius: f32,
    stroke: StrokeKind,
) -> Stateful<Div> {
    let elem_id: gpui::ElementId = id.into();
    let group: SharedString = format!("mi-{elem_id:?}").into();
    components::material_interactive(elem_id, group, fill_hex, radius, stroke)
}

fn avatar(initials: impl Into<SharedString>, bg_hex: u32, size: f32, image: impl Into<components::AvatarImage>) -> Div {
    components::avatar(initials, size, bg_hex, 0xffffff, image)
}

fn alert(
    icon: &'static str,
    color: u32,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    _action: Option<&'static str>,
) -> Div {
    components::alert(icon, title, description, color)
}

const ONBOARDING_STEPS: [(&str, &str); 6] = [
    (
        "What should we call you?",
        "This is your display name — not your username — and you can change it any time in Settings.",
    ),
    (
        "Add a profile picture",
        "Helps your partner recognize you at a glance. Pick a signature color or photo avatar.",
    ),
    (
        "Pick an avatar decoration",
        "Totally optional flair around your avatar. You can always change this later.",
    ),
    (
        "Style your name",
        "How your display name appears in chat, to everyone.",
    ),
    (
        "Pick a look",
        "Sets the color palette for backgrounds, buttons, and other surfaces. Change it any time in Settings.",
    ),
    (
        "You're all set",
        "Here's how you'll show up. You can fine-tune any of this later in Settings.",
    ),
];

const NAME_SWATCHES: [(u32, &str); 10] = [
    (0xf5f5f7, "Default"),
    (0xcbd5e1, "Slate"),
    (0xf87171, "Red"),
    (0xfb923c, "Orange"),
    (0xfbbf24, "Amber"),
    (0x34d399, "Green"),
    (0x22d3ee, "Cyan"),
    (0x60a5fa, "Blue"),
    (0xa78bfa, "Violet"),
    (0xf472b6, "Pink"),
];

const DECORATION_PRESETS: [Option<&str>; 5] = [
    None,
    Some("Aurora Ring"),
    Some("Cyber Pulse"),
    Some("Starlight"),
    Some("Emerald Halo"),
];

fn settings_title(title: &'static str, description: &'static str) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(19.))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(0xf5f5f7))
                .child(title),
        )
        .child(label(description))
}

fn settings_panel() -> Div {
    material_surface(MATERIAL_SUBTLE, 16., StrokeKind::Neutral)
        .p(px(16.))
        .flex()
        .flex_col()
        .gap(px(14.))
}

fn settings_row(title: impl Into<SharedString>, description: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .text_size(px(13.5))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(0xf2f2f3))
                .child(title.into()),
        )
        .child(label(description))
}

fn small_button(id: impl Into<gpui::ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    material_interactive(id, MATERIAL_DARK, 14., StrokeKind::Neutral)
        .h(px(32.))
        .px(px(12.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.5))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(rgb(0xf2f2f3))
        .child(text.into())
}

/// A setting with an on/off button, matching the Chat tab.
fn toggle_row(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    enabled: bool,
) -> (Div, Stateful<Div>) {
    (
        settings_row(title, description),
        material_interactive(
            id,
            if enabled { MATERIAL_GREEN } else { MATERIAL_DARK },
            14.,
            if enabled { StrokeKind::Colored } else { StrokeKind::Neutral },
        )
        .h(px(30.))
        .px(px(12.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .text_size(px(12.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(rgb(0xffffff))
        .child(if enabled { "Enabled" } else { "Disabled" }),
    )
}

fn row(left: Div, right: impl IntoElement) -> Div {
    div().flex().items_center().gap(px(12.)).child(left).child(right)
}

impl Preview {
    #[inline(never)]
    pub fn render_auth_screen(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let stage_bg = self.state.preferences.theme.stage_bg();
        let is_signup = matches!(self.auth_tab, AuthTab::Signup);

        // Segmented Login / Signup Tabs
        self.auth_tabs.select(usize::from(is_signup));
        let tabs_bar = components::tabs(
            "auth-tab",
            vec!["Log in".into(), "Sign up".into()],
            &self.auth_tabs,
            172.,
            window,
            std::rc::Rc::new(cx.listener(|this, index: &usize, _, cx| {
                this.auth_error = None;
                this.auth_tab = if *index == 0 { AuthTab::Login } else { AuthTab::Signup };
                cx.notify();
            })),
        );

        let card_body = match self.auth_tab.clone() {
            AuthTab::VerifyEmail(email) => div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_size(px(18.))
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(rgb(0xf5f5f7))
                                .child("Check your inbox"),
                        )
                        .child(label("Then come back and log in.")),
                )
                .child(alert(
                    "HeroInfoIcon",
                    0x0a84ff,
                    "Verification link sent",
                    format!(
                        "We sent a confirmation link to {}. Click it to activate your account.",
                        email
                    ),
                    None,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            material_interactive(
                                "verify-back",
                                MATERIAL_DARK,
                                12.,
                                StrokeKind::Neutral,
                            )
                            .flex_1()
                            .h(px(38.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(13.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0xf2f2f3))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.auth_tab = AuthTab::Login;
                                cx.notify();
                            }))
                            .child("Back to login"),
                        ),
                ),
            AuthTab::Login => {
                let email_field = material_surface(MATERIAL_DARK, 12., StrokeKind::Neutral)
                    .h(px(40.))
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .child(Input::new(&self.auth_email_input).appearance(false));

                let pass_field = material_surface(MATERIAL_DARK, 12., StrokeKind::Neutral)
                    .h(px(40.))
                    .px(px(4.))
                    .flex()
                    .items_center()
                    .child(
                        Input::new(&self.auth_password_input)
                            
                            .appearance(false),
                    );

                div()
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(tabs_bar)
                    .when_some(self.auth_error.clone(), |d, err| {
                        d.child(alert(
                            "HeroDangerIcon",
                            0xff6467,
                            "Authentication error",
                            err,
                            None,
                        ))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xb4b4be))
                                    .child("Email"),
                            )
                            .child(email_field),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xb4b4be))
                                    .child("Password"),
                            )
                            .child(pass_field),
                    )
                    .child(
                        material_interactive("auth-login-submit", ACCENT, 19., StrokeKind::Colored)
                            .w_full()
                            .h(px(38.))
                            .mt(px(4.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(13.5))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0xffffff))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.submit_login(window, cx);
                            }))
                            .child("Log in"),
                    )
            }
            AuthTab::Signup => {
                let raw_user = self
                    .auth_username_input
                    .read(cx)
                    .value()
                    .trim()
                    .to_lowercase();
                let username_hint = if raw_user.is_empty() {
                    None
                } else if !is_valid_username(&raw_user) {
                    Some(("Only a–z, 0–9 and _, 3–20 characters.", 0xff6467))
                } else {
                    // Whether it's free is only known when signing up, which reports a clash.
                    None
                };

                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(tabs_bar)
                    .when_some(self.auth_error.clone(), |d, err| {
                        d.child(alert("HeroDangerIcon", 0xff6467, "Sign up error", err, None))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xb4b4be))
                                            .child("Username"),
                                    )
                                    .when_some(username_hint, |d, (msg, col)| {
                                        d.child(
                                            div()
                                                .text_size(px(11.))
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .text_color(rgb(col))
                                                .child(msg),
                                        )
                                    }),
                            )
                            .child(
                                material_surface(MATERIAL_DARK, 12., StrokeKind::Neutral)
                                    .h(px(38.))
                                    .px(px(4.))
                                    .flex()
                                    .items_center()
                                    .child(
                                        Input::new(&self.auth_username_input).appearance(false),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xb4b4be))
                                    .child("Display name"),
                            )
                            .child(
                                material_surface(MATERIAL_DARK, 12., StrokeKind::Neutral)
                                    .h(px(38.))
                                    .px(px(4.))
                                    .flex()
                                    .items_center()
                                    .child(
                                        Input::new(&self.auth_display_name_input).appearance(false),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xb4b4be))
                                    .child("Email"),
                            )
                            .child(
                                material_surface(MATERIAL_DARK, 12., StrokeKind::Neutral)
                                    .h(px(38.))
                                    .px(px(4.))
                                    .flex()
                                    .items_center()
                                    .child(Input::new(&self.auth_email_input).appearance(false)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(5.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xb4b4be))
                                    .child("Password"),
                            )
                            .child(
                                material_surface(MATERIAL_DARK, 12., StrokeKind::Neutral)
                                    .h(px(38.))
                                    .px(px(4.))
                                    .flex()
                                    .items_center()
                                    .child(
                                        Input::new(&self.auth_password_input)
                                            
                                            .appearance(false),
                                    ),
                            ),
                    )
                    .child(
                        material_interactive(
                            "auth-signup-submit",
                            MATERIAL_GREEN,
                            19.,
                            StrokeKind::Colored,
                        )
                        .w_full()
                        .h(px(38.))
                        .mt(px(4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(13.5))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0xffffff))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.submit_signup(window, cx);
                        }))
                        .child("Create account"),
                    )
            }
        };

        div()
            .size_full()
            .relative()
            .bg(rgb(stage_bg))
            .flex()
            .items_center()
            .justify_center()
            .p(px(24.))
            .child(
                material_surface(MATERIAL_PANEL, 20., StrokeKind::Neutral)
                    .w(px(400.))
                    .p(px(24.))
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(16.))
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xf5f5f7))
                                            .child("N I T R O"),
                                    )
                                    .child(badge("GPUI v3", 0x26262b, 0xa1a1aa)),
                            )
                            .child(label("Quiet, one-on-one conversations.")),
                    )
                    .child(card_body)
                    .when_some(self.auth_busy.clone(), |d, message| d.child(label(message))),
            )
    }

    #[inline(never)]
    pub fn render_onboarding_screen(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let stage_bg = self.state.preferences.theme.stage_bg();
        let step = self.onboarding_step.min(ONBOARDING_STEPS.len() - 1);
        let (title, description) = ONBOARDING_STEPS[step];

        let mut progress_dots = div().flex().items_center().gap(px(6.)).w_full();
        for idx in 0..ONBOARDING_STEPS.len() {
            let dot_color = if idx < step {
                MATERIAL_GREEN
            } else if idx == step {
                0xa1a1aa
            } else {
                0x2e2e34
            };
            progress_dots = progress_dots.child(
                div()
                    .flex_1()
                    .h(px(4.))
                    .rounded_full()
                    .bg(rgb(dot_color)),
            );
        }

        let step_body = match step {
            0 => div()
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0xb4b4be))
                        .child("DISPLAY NAME"),
                )
                .child(
                    material_surface(MATERIAL_DARK, 14., StrokeKind::Neutral)
                        .h(px(46.))
                        .px(px(8.))
                        .flex()
                        .items_center()
                        .child(Input::new(&self.onboarding_name_input).appearance(false)),
                ),
            1 => {
                let mut color_row = div().flex().flex_wrap().gap(px(10.));
                for (col, col_name) in [
                    (0x0a84ff, "Nitro Blue"),
                    (0x00b339, "Signal Green"),
                    (0x8b5cf6, "Royal Violet"),
                    (0xec4899, "Neon Rose"),
                    (0xf59e0b, "Solar Amber"),
                ] {
                    let active =
                        self.state.profile.avatar_color == col;
                    color_row = color_row.child(
                        material_interactive(
                            ("onb-av-col", col as usize),
                            if active { col } else { MATERIAL_DARK },
                            14.,
                            if active {
                                StrokeKind::Colored
                            } else {
                                StrokeKind::Neutral
                            },
                        )
                        .h(px(36.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.profile.avatar_color = col;
                            cx.notify();
                        }))
                        .child(div().size(px(12.)).rounded_full().bg(rgb(col)))
                        .child(
                            div()
                                .text_size(px(12.5))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(rgb(0xf2f2f3))
                                .child(col_name),
                        ),
                    );
                }
                div()
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .child(
                        material_surface(MATERIAL_SUBTLE, 16., StrokeKind::Neutral)
                            .p(px(18.))
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .child(avatar(
                                self.state.profile.initials.clone(),
                                self.state.profile.avatar_color,
                                64.,
                                self.state.profile.photo.clone(),
                            ))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_size(px(16.))
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(self.state.profile.name_color))
                                            .child(self.state.profile.display_name.clone()),
                                    )
                                    .child(label(format!("@{}", self.state.profile.username))),
                            ),
                    )
                    .child(color_row)
            }
            2 => {
                let mut deco_grid = div().flex().flex_wrap().gap(px(10.));
                for (idx, preset) in DECORATION_PRESETS.into_iter().enumerate() {
                    let label_txt = preset.unwrap_or("None");
                    let active =
                        self.state.profile.avatar_decoration.as_deref() == preset;
                    deco_grid = deco_grid.child(
                        material_interactive(
                            ("onb-deco", idx),
                            if active { ACCENT } else { MATERIAL_DARK },
                            14.,
                            if active {
                                StrokeKind::Colored
                            } else {
                                StrokeKind::Neutral
                            },
                        )
                        .h(px(40.))
                        .px(px(14.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(rgb(0xf2f2f3))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.profile.avatar_decoration =
                                preset.map(|s| s.to_string());
                            cx.notify();
                        }))
                        .child(label_txt),
                    );
                }
                deco_grid
            }
            3 => {
                let mut swatches = div().flex().flex_wrap().gap(px(8.));
                for (idx, (col, col_label)) in NAME_SWATCHES.into_iter().enumerate() {
                    let active = self.state.profile.name_color == col;
                    swatches = swatches.child(
                        material_interactive(
                            ("onb-swatch", idx),
                            if active { 0x34343a } else { MATERIAL_DARK },
                            12.,
                            if active {
                                StrokeKind::AccentFocus
                            } else {
                                StrokeKind::Neutral
                            },
                        )
                        .h(px(36.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.profile.name_color = col;
                            cx.notify();
                        }))
                        .child(div().size(px(12.)).rounded_full().bg(rgb(col)))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(0xe4e4e7))
                                .child(col_label),
                        ),
                    );
                }
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(
                        material_surface(MATERIAL_SUBTLE, 14., StrokeKind::Neutral)
                            .h(px(64.))
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(self.state.profile.name_color))
                                    .child(self.state.profile.display_name.clone()),
                            ),
                    )
                    .child(swatches)
            }
            4 => {
                let mut theme_grid = div().flex().flex_wrap().gap(px(12.));
                for (idx, theme_opt) in AppTheme::ALL.into_iter().enumerate() {
                    let active = self.state.preferences.theme == theme_opt;
                    theme_grid = theme_grid.child(
                        material_interactive(
                            ("onb-theme", idx),
                            if active { 0x303036 } else { MATERIAL_DARK },
                            14.,
                            if active {
                                StrokeKind::AccentFocus
                            } else {
                                StrokeKind::Neutral
                            },
                        )
                        .w(px(148.))
                        .p(px(12.))
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.preferences.theme = theme_opt;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .h(px(44.))
                                .w_full()
                                .rounded(px(10.))
                                .bg(gpui::rgb(theme_opt.stage_bg()))
                                .p(px(8.))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .size(px(20.))
                                        .rounded_full()
                                        .bg(gpui::rgb(theme_opt.accent_dot())),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .h(px(8.))
                                        .rounded_full()
                                        .bg(gpui::rgb(theme_opt.preview_bar())),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(rgb(0xf2f2f3))
                                        .child(theme_opt.label()),
                                )
                                .when(active, |d| d.child(badge("Active", ACCENT, 0xffffff))),
                        ),
                    );
                }
                theme_grid
            }
            _ => material_surface(MATERIAL_SUBTLE, 16., StrokeKind::Neutral)
                .p(px(16.))
                .flex()
                .items_start()
                .gap(px(12.))
                .child(avatar(
                    self.state.profile.initials.clone(),
                    self.state.profile.avatar_color,
                    42.,
                    self.state.profile.photo.clone(),
                ))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_size(px(14.5))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(self.state.profile.name_color))
                                        .child(self.state.profile.display_name.clone()),
                                )
                                .when_some(
                                    self.state.profile.avatar_decoration.clone(),
                                    |d, deco| d.child(badge(deco, 0x26262b, 0xa1a1aa)),
                                )
                                .child(label("Just now")),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .text_color(rgb(0xe4e4e7))
                                .child("Hey! Ready to jump into voice and test the new Nitro GPUI shell."),
                        ),
                ),
        };

        div()
            .size_full()
            .bg(rgb(stage_bg))
            .flex()
            .justify_center()
            .child(
                div()
                    .w(px(540.))
                    .h_full()
                    .pt(px(84.))
                    .pb(px(40.))
                    .px(px(32.))
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(22.))
                            .child(progress_dots)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(
                                        div()
                                            .text_size(px(24.))
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xf5f5f7))
                                            .child(title),
                                    )
                                    .child(label(description)),
                            )
                            .child(step_body),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pt(px(18.))
                            .child(if step > 0 {
                                material_interactive(
                                    "onb-back",
                                    MATERIAL_DARK,
                                    18.,
                                    StrokeKind::Neutral,
                                )
                                .h(px(36.))
                                .px(px(18.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(13.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(0xe4e4e7))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.onboarding_step = this.onboarding_step.saturating_sub(1);
                                    cx.notify();
                                }))
                                .child("Back")
                                .into_any_element()
                            } else {
                                div().into_any_element()
                            })
                            .child(
                                material_interactive(
                                    "onb-next",
                                    if step == ONBOARDING_STEPS.len() - 1 {
                                        MATERIAL_GREEN
                                    } else {
                                        0xf2f2f3
                                    },
                                    18.,
                                    StrokeKind::Colored,
                                )
                                .h(px(36.))
                                .px(px(22.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(13.))
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(rgb(if step == ONBOARDING_STEPS.len() - 1 {
                                    0xffffff
                                } else {
                                    0x111113
                                }))
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    if step == 0 {
                                        let entered = this
                                            .onboarding_name_input
                                            .read(cx)
                                            .value()
                                            .trim()
                                            .to_string();
                                        if !entered.is_empty() {
                                            this.state.profile.display_name = entered.clone();
                                            this.state.profile.initials = initials_for(&entered);
                                        }
                                    }
                                    if step + 1 < ONBOARDING_STEPS.len() {
                                        this.onboarding_step += 1;
                                    } else {
                                        this.finish_onboarding(cx);
                                    }
                                    cx.notify();
                                }))
                                .child(if step == ONBOARDING_STEPS.len() - 1 {
                                    "Finish"
                                } else {
                                    "Continue"
                                }),
                            ),
                    ),
            )
    }

    #[inline(never)]
    pub fn render_dashboard_view(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let incoming: Vec<(usize, FriendRequest)> = self
            .state
            .friend_requests
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, r)| r.incoming)
            .collect();
        let outgoing: Vec<(usize, FriendRequest)> = self
            .state
            .friend_requests
            .iter()
            .cloned()
            .enumerate()
            .filter(|(_, r)| !r.incoming)
            .collect();

        let mut incoming_list = div().flex().flex_col().gap(px(6.));
        for (req_idx, req) in &incoming {
            let idx = *req_idx;
            let accept_id = req.id.clone();
            let decline_id = req.id.clone();
            incoming_list = incoming_list.child(
                material_surface(MATERIAL_SUBTLE, 14., StrokeKind::Neutral)
                    .px(px(12.))
                    .py(px(9.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(11.))
                            .child(avatar(req.initials.clone(), req.color, 36., false))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xf2f2f3))
                                            .child(req.display_name.clone()),
                                    )
                                    .child(label(format!("@{}", req.username))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                material_interactive(
                                    ("accept-friend", idx),
                                    MATERIAL_GREEN,
                                    15.,
                                    StrokeKind::Colored,
                                )
                                .h(px(30.))
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(px(12.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(0xffffff))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.accept_friend(accept_id.clone(), cx);
                                }))
                                .child("Accept"),
                            )
                            .child(
                                material_interactive(
                                    ("decline-friend", idx),
                                    MATERIAL_DARK,
                                    15.,
                                    StrokeKind::Neutral,
                                )
                                .size(px(30.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.remove_friendship(decline_id.clone(), cx);
                                }))
                                .child(
                                    svg()
                                        .path("WindowClose")
                                        .size(px(11.))
                                        .text_color(rgb(0xd4d4d8)),
                                ),
                            ),
                    ),
            );
        }

        let mut outgoing_list = div().flex().flex_col().gap(px(6.));
        for (req_idx, req) in &outgoing {
            let idx = *req_idx;
            let cancel_id = req.id.clone();
            outgoing_list = outgoing_list.child(
                material_surface(MATERIAL_SUBTLE, 14., StrokeKind::Neutral)
                    .px(px(12.))
                    .py(px(9.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(11.))
                            .child(avatar(req.initials.clone(), req.color, 36., false))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(14.))
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xf2f2f3))
                                            .child(req.display_name.clone()),
                                    )
                                    .child(label("Outgoing request · Pending")),
                            ),
                    )
                    .child(
                        material_interactive(
                            ("cancel-friend", idx),
                            MATERIAL_DARK,
                            15.,
                            StrokeKind::Neutral,
                        )
                        .h(px(28.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .text_size(px(11.5))
                        .text_color(rgb(0xd4d4d8))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_friendship(cancel_id.clone(), cx);
                        }))
                        .child("Cancel"),
                    ),
            );
        }

        let mut all_chats = div().flex().flex_col().gap(px(6.));
        for (c_idx, conv) in self.state.conversations.iter().enumerate() {
            let is_active = self.state.active_partner == c_idx;
            let last_excerpt = conv
                .messages
                .last()
                .map(|m| {
                    if m.deleted {
                        "Message deleted".to_string()
                    } else if m.text.is_empty() {
                        "Attachment".to_string()
                    } else {
                        m.text.clone()
                    }
                })
                .unwrap_or_else(|| conv.preview.clone());
            let presence_col = match conv.presence.as_str() {
                "online" => 0x00d084,
                "away" => 0xf5c344,
                _ => 0x6b6b6b,
            };

            all_chats = all_chats.child(
                material_interactive(
                    ("dash-conv", c_idx),
                    if is_active { 0x2a2a30 } else { MATERIAL_SUBTLE },
                    14.,
                    StrokeKind::Neutral,
                )
                .px(px(12.))
                .py(px(10.))
                .flex()
                .items_center()
                .justify_between()
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.switch_partner(c_idx, window, cx);
                    this.shell_view = ShellView::Chat;
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(11.))
                        .child(
                            div()
                                .relative()
                                .size(px(38.))
                                .child(avatar(
                                    conv.initials.clone(),
                                    conv.color,
                                    38.,
                                    conv.photo.clone(),
                                ))
                                .child(
                                    div()
                                        .absolute()
                                        .right(px(-1.))
                                        .bottom(px(-1.))
                                        .size(px(11.))
                                        .rounded_full()
                                        .bg(rgb(presence_col))
                                        .border_2()
                                        .border_color(rgb(0x141416)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(0xf2f2f3))
                                        .child(components::name_text(conv.display_name.clone(), conv.name_effect, 14.)),
                                )
                                .child(label(last_excerpt)),
                        ),
                )
                .when(conv.unread_count > 0, |d| {
                    d.child(badge(
                        format!("{}", conv.unread_count),
                        MATERIAL_GREEN,
                        0xffffff,
                    ))
                }),
            );
        }

        div()
            .id("v3-dashboard-scroll")
            .size_full()
            .overflow_y_scroll()
            .pt(px(108.))
            .pb(px(48.))
            .px(px(30.))
            .flex()
            .justify_center()
            .child(
                div()
                    .w(px(560.))
                    .flex()
                    .flex_col()
                    .gap(px(24.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(20.))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xf5f5f7))
                                    .child("Dashboard"),
                            )
                            .child(
                                material_interactive(
                                    "dashboard-back-to-chat",
                                    0x2c2d2d,
                                    17.,
                                    StrokeKind::Neutral,
                                )
                                .h(px(34.))
                                .px(px(14.))
                                .flex()
                                .items_center()
                                .gap(px(7.))
                                .text_size(px(12.5))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(0xf2f2f3))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.shell_view = ShellView::Chat;
                                    cx.notify();
                                }))
                                .child("Back to chat"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0x8e8e98))
                                    .child("ADD A FRIEND"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        material_surface(MATERIAL_DARK, 12., StrokeKind::Neutral)
                                            .flex_1()
                                            .h(px(40.))
                                            .px(px(8.))
                                            .flex()
                                            .items_center()
                                            .child(
                                                Input::new(&self.add_friend_input)
                                                    .appearance(false),
                                            ),
                                    )
                                    .child(
                                        material_interactive(
                                            "send-friend-req",
                                            0xf2f2f3,
                                            12.,
                                            StrokeKind::Colored,
                                        )
                                        .h(px(40.))
                                        .px(px(16.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(13.))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(rgb(0x111113))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            let raw = this
                                                .add_friend_input
                                                .read(cx)
                                                .value()
                                                .trim()
                                                .to_lowercase();
                                            if !raw.is_empty() {
                                                this.add_friend_input.update(cx, |s, cx| {
                                                    s.set_value("", window, cx);
                                                });
                                                this.request_friend(raw, cx);
                                            }
                                        }))
                                        .child("Send"),
                                    ),
                            ),
                    )
                    .when_some(self.friends_error.clone(), |d, message| {
                        d.child(components::alert(
                            "HeroDangerIcon",
                            "Couldn't do that",
                            message,
                            MATERIAL_RED,
                        ))
                    })
                    .when(!incoming.is_empty(), |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(rgb(0x8e8e98))
                                        .child("FRIEND REQUESTS"),
                                )
                                .child(incoming_list),
                        )
                    })
                    .when(!outgoing.is_empty(), |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(rgb(0x8e8e98))
                                        .child("PENDING"),
                                )
                                .child(outgoing_list),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0x8e8e98))
                                    .child(format!(
                                        "ALL CHATS — {}",
                                        self.state.conversations.len()
                                    )),
                            )
                            .child(all_chats),
                    ),
            )
    }

    #[inline(never)]
    pub fn render_settings_view(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let tabs: [(SettingsTab, &str); 7] = [
            (SettingsTab::General, "My Account"),
            (SettingsTab::Appearance, "Appearance"),
            (SettingsTab::Chat, "Chat"),
            (SettingsTab::Voice, "Voice & Video"),
            (SettingsTab::Keybinds, "Keybinds"),
            (SettingsTab::Soundboard, "Soundboard"),
            (SettingsTab::About, "About"),
        ];

        let mut sidebar_nav = div().w(px(230.)).flex().flex_col().gap(px(4.)).px(px(14.));
        for (idx, (t_val, t_label)) in tabs.into_iter().enumerate() {
            let active = self.settings_tab == t_val;
            let row = if active {
                material_surface(MATERIAL_DARK, 14., StrokeKind::Neutral)
                    .h(px(40.))
                    .px(px(12.))
                    .flex()
                    .items_center()
            } else {
                div()
                    .h(px(40.))
                    .px(px(12.))
                    .rounded(px(14.))
                    .flex()
                    .items_center()
                    .hover(|s| s.bg(rgba(0xffffff0f)))
            };
            sidebar_nav = sidebar_nav.child(
                row.id(("settings-nav", idx))
                    .cursor_pointer()
                    .text_size(px(13.5))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(rgb(if active { 0xffffff } else { 0xa1a1aa }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings_tab = t_val;
                        cx.notify();
                    }))
                    .child(t_label),
            );
        }

        sidebar_nav = sidebar_nav
            .child(div().h(px(1.)).my(px(8.)).bg(rgba(0xffffff14)))
            .child(
                material_interactive(
                    "settings-logout-btn",
                    MATERIAL_RED,
                    14.,
                    StrokeKind::Colored,
                )
                .h(px(38.))
                .px(px(12.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(0xffffff))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.sign_out(cx);
                }))
                .child("Log out"),
            );

        let update_text: SharedString = match &self.update {
            crate::UpdateState::Idle => "Native desktop app".into(),
            crate::UpdateState::Checking => "Checking for updates…".into(),
            crate::UpdateState::UpToDate => "You're on the latest version.".into(),
            crate::UpdateState::Available(update) => format!("Nitro {} is available.", update.version).into(),
            crate::UpdateState::Downloading(version) => format!(
                "Downloading Nitro {version}… {}%",
                self.update_progress.load(std::sync::atomic::Ordering::Relaxed) / 10
            )
            .into(),
            crate::UpdateState::Failed(message) => message.clone().into(),
        };
        let update_button = match &self.update {
            crate::UpdateState::Checking | crate::UpdateState::Downloading(_) => None,
            crate::UpdateState::Available(update) => {
                let update = update.clone();
                Some(
                    material_interactive("about-install-update", ACCENT, 16., StrokeKind::Colored)
                        .h(px(34.))
                        .px(px(14.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0xffffff))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.install_update(update.clone(), cx);
                        }))
                        .child("Install and restart"),
                )
            }
            _ => Some(
                material_interactive("about-check-update", MATERIAL_DARK, 16., StrokeKind::Neutral)
                    .h(px(34.))
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(0xf2f2f3))
                    .on_click(cx.listener(|this, _, _, cx| this.check_for_update(true, cx)))
                    .child("Check for updates"),
            ),
        };
        let panel_content = match self.settings_tab {
            SettingsTab::General => {
                self.ensure_decorations(cx);
                let section_title = |title: &'static str| {
                    div()
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(0xf2f2f3))
                        .child(title)
                };
                let chip_fill = |active: bool| if active { 0x323238 } else { MATERIAL_DARK };
                let chip_stroke = |active: bool| {
                    if active { StrokeKind::AccentFocus } else { StrokeKind::Neutral }
                };

                let my_decoration = self.state.profile.avatar_decoration.clone();
                let my_photo = self.state.profile.photo.path.clone();
                let mut decorations = div().flex().flex_wrap().gap(px(8.)).child(
                    material_interactive(
                        "settings-decoration-none",
                        chip_fill(my_decoration.is_none()),
                        12.,
                        chip_stroke(my_decoration.is_none()),
                    )
                    .size(px(68.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .text_color(rgb(0xe4e4e7))
                    .on_click(cx.listener(|this, _, _, cx| this.set_decoration(None, cx)))
                    .child("None"),
                );
                for (index, id) in self
                    .decoration_catalog
                    .iter()
                    .take(self.decorations_shown)
                    .enumerate()
                {
                    let active = my_decoration.as_deref() == Some(id.as_str());
                    let chosen = id.clone();
                    decorations = decorations.child(
                        material_interactive(
                            ("settings-decoration", index),
                            chip_fill(active),
                            12.,
                            chip_stroke(active),
                        )
                        .size(px(68.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_decoration(Some(chosen.clone()), cx);
                        }))
                        .child(avatar(
                            self.state.profile.initials.clone(),
                            self.state.profile.avatar_color,
                            40.,
                            components::AvatarImage {
                                path: my_photo.clone(),
                                decoration: self.decoration_files.get(id).cloned().flatten(),
                            },
                        )),
                    );
                }
                let more_decorations = self.decoration_catalog.len() > self.decorations_shown;

                let my_effect = self.state.profile.name_effect;
                let mut effects = div().flex().flex_wrap().gap(px(8.)).child(
                    material_interactive(
                        "settings-effect-none",
                        chip_fill(my_effect.is_none()),
                        10.,
                        chip_stroke(my_effect.is_none()),
                    )
                    .h(px(34.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .text_size(px(12.5))
                    .text_color(rgb(0xe4e4e7))
                    .on_click(cx.listener(|this, _, _, cx| this.set_name_effect(None, cx)))
                    .child("None"),
                );
                for (index, effect) in components::NameEffect::ALL.into_iter().enumerate() {
                    let active = my_effect == Some(effect);
                    effects = effects.child(
                        material_interactive(
                            ("settings-effect", index),
                            chip_fill(active),
                            10.,
                            chip_stroke(active),
                        )
                        .h(px(34.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .text_size(px(12.5))
                        .text_color(rgb(0xe4e4e7))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_name_effect(Some(effect), cx);
                        }))
                        .child(components::name_text(effect.label(), Some(effect), 12.5)),
                    );
                }

                let my_weight = self.state.profile.name_weight_value;
                let mut weights = div().flex().flex_wrap().gap(px(8.));
                for (index, (_, weight, name)) in
                    crate::backend::NAME_WEIGHTS.into_iter().enumerate()
                {
                    let active = my_weight == weight;
                    weights = weights.child(
                        material_interactive(
                            ("settings-weight", index),
                            chip_fill(active),
                            10.,
                            chip_stroke(active),
                        )
                        .h(px(34.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .text_size(px(12.5))
                        .font_weight(gpui::FontWeight(weight as f32))
                        .text_color(rgb(0xe4e4e7))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_name_weight(weight, cx);
                        }))
                        .child(name),
                    );
                }

                let mut swatches = div().flex().flex_wrap().gap(px(8.));
                for (idx, (col, name)) in NAME_SWATCHES.into_iter().enumerate() {
                    let active = self.state.profile.name_color == col;
                    swatches = swatches.child(
                        material_interactive(
                            ("settings-swatch", idx),
                            if active { 0x323238 } else { MATERIAL_DARK },
                            10.,
                            if active {
                                StrokeKind::AccentFocus
                            } else {
                                StrokeKind::Neutral
                            },
                        )
                        .h(px(34.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_name_color(col, cx);
                        }))
                        .child(div().size(px(12.)).rounded_full().bg(rgb(col)))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(0xe4e4e7))
                                .child(name),
                        ),
                    );
                }

                div()
                    .flex()
                    .flex_col()
                    .gap(px(20.))
                    .max_w(px(620.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(19.))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xf5f5f7))
                                    .child("My Account"),
                            )
                            .child(label(
                                "Manage your profile, display name, and live appearance in 1-on-1 chats.",
                            )),
                    )
                    .child(
                        material_surface(MATERIAL_SUBTLE, 16., StrokeKind::Neutral)
                            .p(px(18.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(16.))
                                    .child(avatar(
                                        self.state.profile.initials.clone(),
                                        self.state.profile.avatar_color,
                                        64.,
                                        self.state.profile.photo.clone(),
                                    ))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .text_size(px(18.))
                                                    .font_weight(gpui::FontWeight::BOLD)
                                                    .text_color(rgb(self.state.profile.name_color))
                                                    .child(self.state.profile.display_name.clone()),
                                            )
                                            .child(label(format!(
                                                "@{} · {}",
                                                self.state.profile.username,
                                                self.state.profile.email
                                            ))),
                                    ),
                            ),
                    )
                    .child(
                        material_surface(MATERIAL_SUBTLE, 16., StrokeKind::Neutral)
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xf2f2f3))
                                    .child("Name color"),
                            )
                            .child(swatches),
                    )
                    .child(
                        material_surface(MATERIAL_SUBTLE, 16., StrokeKind::Neutral)
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child(section_title("Name effect"))
                            .child(effects)
                            .child(section_title("Name weight"))
                            .child(weights),
                    )
                    .child(
                        material_surface(MATERIAL_SUBTLE, 16., StrokeKind::Neutral)
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .child(section_title("Avatar decoration"))
                            .child(decorations)
                            .when(more_decorations, |card| {
                                card.child(
                                    material_interactive(
                                        "settings-decorations-more",
                                        MATERIAL_DARK,
                                        10.,
                                        StrokeKind::Neutral,
                                    )
                                    .h(px(34.))
                                    .px(px(12.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.5))
                                    .text_color(rgb(0xe4e4e7))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.decorations_shown += 24;
                                        cx.notify();
                                    }))
                                    .child("Show more"),
                                )
                            }),
                    )
            }
            SettingsTab::Appearance => {
                let mut theme_cards = div().flex().flex_wrap().gap(px(12.));
                for (idx, theme_opt) in AppTheme::ALL.into_iter().enumerate() {
                    let active = self.state.preferences.theme == theme_opt;
                    theme_cards = theme_cards.child(
                        material_interactive(
                            ("settings-theme", idx),
                            if active { 0x303036 } else { MATERIAL_DARK },
                            14.,
                            if active {
                                StrokeKind::AccentFocus
                            } else {
                                StrokeKind::Neutral
                            },
                        )
                        .w(px(160.))
                        .p(px(12.))
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.preferences.theme = theme_opt;
                            this.persist();
                            cx.notify();
                        }))
                        .child(
                            div()
                                .h(px(48.))
                                .w_full()
                                .rounded(px(10.))
                                .bg(gpui::rgb(theme_opt.stage_bg()))
                                .p(px(10.))
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .size(px(22.))
                                        .rounded_full()
                                        .bg(gpui::rgb(theme_opt.accent_dot())),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .h(px(10.))
                                        .rounded_full()
                                        .bg(gpui::rgb(theme_opt.preview_bar())),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(rgb(0xf2f2f3))
                                        .child(theme_opt.label()),
                                )
                                .when(active, |d| d.child(badge("Active", ACCENT, 0xffffff))),
                        ),
                    );
                }

                div()
                    .flex()
                    .flex_col()
                    .gap(px(20.))
                    .max_w(px(620.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(px(19.))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0xf5f5f7))
                                    .child("Appearance"),
                            )
                            .child(label(
                                "Choose a surface palette for the stage, floating pills, and transparent sidebar.",
                            )),
                    )
                    .child(theme_cards)
            }
            SettingsTab::Voice => {
                let mut in_list = div().flex().flex_col().gap(px(6.));
                for (idx, dev) in self.audio_devices.inputs.iter().enumerate() {
                    let selected = self.state.preferences.input_device_id == dev.id;
                    let dev_id = dev.id.clone();
                    in_list = in_list.child(
                        material_interactive(
                            ("settings-in-dev", idx),
                            if selected { ACCENT } else { MATERIAL_DARK },
                            10.,
                            if selected {
                                StrokeKind::Colored
                            } else {
                                StrokeKind::Neutral
                            },
                        )
                        .h(px(34.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(12.5))
                        .text_color(rgb(0xf2f2f3))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.preferences.input_device_id = dev_id.clone();
                                this.send_voice_routes();
                            this.persist();
                            cx.notify();
                        }))
                        .child(dev.name.clone())
                        .when(selected, |d| d.child("✓")),
                    );
                }

                let mut out_list = div().flex().flex_col().gap(px(6.));
                for (idx, dev) in self.audio_devices.outputs.iter().enumerate() {
                    let selected = self.state.preferences.output_device_id == dev.id;
                    let dev_id = dev.id.clone();
                    out_list = out_list.child(
                        material_interactive(
                            ("settings-out-dev", idx),
                            if selected { ACCENT } else { MATERIAL_DARK },
                            10.,
                            if selected {
                                StrokeKind::Colored
                            } else {
                                StrokeKind::Neutral
                            },
                        )
                        .h(px(34.))
                        .px(px(12.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_size(px(12.5))
                        .text_color(rgb(0xf2f2f3))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.state.preferences.output_device_id = dev_id.clone();
                                this.send_voice_routes();
                            this.persist();
                            cx.notify();
                        }))
                        .child(dev.name.clone())
                        .when(selected, |d| d.child("✓")),
                    );
                }

                let prefs = self.state.preferences.clone();
                let (sounds_copy, sounds_button) = toggle_row(
                    "settings-interface-sounds",
                    "Interface sounds",
                    "Button, menu, mute and call sounds.",
                    prefs.interface_sounds,
                );
                let (overlay_copy, overlay_button) = toggle_row(
                    "settings-overlay",
                    "Call overlay",
                    "Shows who's in the call and speaking, on top of other apps.",
                    prefs.voice_hud_enabled,
                );
                let (names_copy, names_button) = toggle_row(
                    "settings-overlay-names",
                    "Show names on overlay",
                    "Adds display names next to the avatars.",
                    prefs.voice_hud_show_names,
                );
                let audio_sliders = settings_panel()
                    .child(components::slider_field(
                        "Interface sound volume",
                        format!("{}%", prefs.interface_sound_volume),
                        560.,
                        self.slider_for(6, prefs.interface_sound_volume, 100, 5, 560., cx),
                    ))
                    .child(components::slider_field(
                        "Your soundboard volume",
                        format!("{}%", prefs.soundboard_volume),
                        560.,
                        self.slider_for(2, prefs.soundboard_volume, 100, 5, 560., cx),
                    ));
                let overlay_panel = settings_panel()
                    .child(row(
                        overlay_copy,
                        overlay_button.on_click(cx.listener(|this, _, _, cx| {
                            this.state.preferences.voice_hud_enabled =
                                !this.state.preferences.voice_hud_enabled;
                            this.persist();
                            cx.notify();
                        })),
                    ))
                    .child(row(
                        names_copy,
                        names_button.on_click(cx.listener(|this, _, _, cx| {
                            this.state.preferences.voice_hud_show_names =
                                !this.state.preferences.voice_hud_show_names;
                            this.persist();
                            cx.notify();
                        })),
                    ))
                    .child(components::slider_field(
                        "Overlay size",
                        format!("{}%", prefs.voice_hud_scale),
                        560.,
                        self.slider_for(7, prefs.voice_hud_scale, 150, 10, 560., cx),
                    ))
                    .child(row(
                        sounds_copy,
                        sounds_button.on_click(cx.listener(|this, _, _, cx| {
                            this.state.preferences.interface_sounds =
                                !this.state.preferences.interface_sounds;
                            this.persist();
                            cx.notify();
                        })),
                    ));

                div()
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .max_w(px(620.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .text_size(px(19.))
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .text_color(rgb(0xf5f5f7))
                                            .child("Voice & Video"),
                                    )
                                    .child(label(
                                        "Select hardware input/output devices and default voice levels.",
                                    )),
                            )
                            .child(
                                material_interactive(
                                    "settings-refresh-devices",
                                    MATERIAL_DARK,
                                    12.,
                                    StrokeKind::Neutral,
                                )
                                .h(px(32.))
                                .px(px(12.))
                                .flex()
                                .items_center()
                                .text_size(px(12.))
                                .text_color(rgb(0xe4e4e7))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.refresh_audio_devices(cx);
                                    cx.notify();
                                }))
                                .child("Rescan devices"),
                            ),
                    )
                    .child(
                        material_surface(MATERIAL_SUBTLE, 16., StrokeKind::Neutral)
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0x8e8e98))
                                    .child("INPUT DEVICE (MICROPHONE)"),
                            )
                            .child(in_list),
                    )
                    .child(
                        material_surface(MATERIAL_SUBTLE, 16., StrokeKind::Neutral)
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(rgb(0x8e8e98))
                                    .child("OUTPUT DEVICE (HEADPHONES / SPEAKERS)"),
                            )
                            .child(out_list),
                    )
                    .child(audio_sliders)
                    .child(overlay_panel)
            }
            SettingsTab::Keybinds => {
                let mut rows = settings_panel();
                for (index, action) in crate::state::KEYBIND_ACTIONS.into_iter().enumerate() {
                    let recording = self.recording_keybind == Some(index);
                    let bind = self.state.preferences.keybinds[index].clone();
                    rows = rows.child(row(
                        settings_row(action, "Works while Nitro is focused and you're in a call."),
                        small_button(
                            ("settings-keybind", index),
                            if recording {
                                "Press a key… (Esc cancels)".to_string()
                            } else if bind.is_empty() {
                                "Not set".to_string()
                            } else {
                                bind.to_uppercase()
                            },
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.recording_keybind = Some(index);
                            cx.notify();
                        })),
                    ));
                }
                div()
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .max_w(px(620.))
                    .child(settings_title(
                        "Keybinds",
                        "Shortcuts work anywhere inside Nitro. Recording a duplicate replaces the old binding.",
                    ))
                    .child(rows)
            }
            SettingsTab::Soundboard => {
                if matches!(self.soundboard_status, crate::SoundboardStatus::NotLoaded) {
                    self.load_soundboard(cx);
                }
                const LIMIT: u64 = 16 * 1024 * 1024;
                let mine: Vec<_> = self.soundboard_clips.iter().filter(|clip| clip.is_mine).cloned().collect();
                let used: u64 = mine.iter().map(|clip| clip.size_bytes).sum();
                let megabytes = |bytes: u64| bytes as f32 / (1024. * 1024.);
                let fill = (used as f32 / LIMIT as f32).clamp(0., 1.);
                let storage = settings_panel()
                    .child(row(
                        settings_row(
                            "Your sounds",
                            format!(
                                "{} sounds · {:.1} of {:.0} MB used",
                                mine.len(),
                                megabytes(used),
                                megabytes(LIMIT)
                            ),
                        ),
                        div(),
                    ))
                    .child(
                        div()
                            .w_full()
                            .h(px(6.))
                            .rounded_full()
                            .overflow_hidden()
                            .bg(rgb(0x34343a))
                            .child(
                                div()
                                    .h_full()
                                    .w(gpui::relative(fill))
                                    .bg(rgb(components::ACCENT)),
                            ),
                    );
                let time = |ms: u32| {
                    let seconds = ms as f32 / 1000.;
                    format!("{}:{:04.1}", (seconds / 60.) as u32, seconds % 60.)
                };
                fn sound_button(id: &'static str, text: &'static str, primary: bool) -> gpui::Stateful<gpui::Div> {
                    material_interactive(
                        id,
                        if primary { components::ACCENT } else { MATERIAL_DARK },
                        16.,
                        if primary { StrokeKind::Colored } else { StrokeKind::Neutral },
                    )
                    .h(px(34.))
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(if primary { 0xffffff } else { 0xf2f2f3 }))
                    .child(text)
                }
                let add_panel = if let Some(draft) = &self.sound_draft {
                    let selected = draft.end_ms - draft.start_ms;
                    let valid = (crate::sound_file::MIN_CLIP_MS..=crate::sound_file::MAX_CLIP_MS).contains(&selected);
                    let unit = draft.trim_unit();
                    let previewing = self.playing_sound_id.as_deref() == Some("sound-draft-preview");
                    let steps = (draft.duration_ms / unit).max(1) as u16;
                    settings_panel()
                        .child(row(
                            settings_row(
                                draft.file_name.clone(),
                                format!("Original {} · selected {}", time(draft.duration_ms), time(selected)),
                            ),
                            sound_button("sound-draft-discard", "Discard", false)
                                .on_click(cx.listener(|this, _, _, cx| this.discard_sound_draft(cx))),
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(6.))
                                .child(label("Sound name"))
                                .child(
                                    material_surface(MATERIAL_DARK, 12., StrokeKind::Neutral)
                                        .px(px(10.))
                                        .py(px(4.))
                                        .child(Input::new(&self.sound_draft_name).appearance(false)),
                                ),
                        )
                        .child(row(
                            div().flex_1().child(label("Trim clip")),
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(if valid { 0xa1a1aa } else { 0xff6467 }))
                                .child(if valid { "Ready to save" } else { "15 seconds max" }),
                        ))
                        .child(components::slider_field(
                            "Start",
                            time(draft.start_ms),
                            560.,
                            components::slider(
                                8,
                                &self.sliders[8],
                                (draft.start_ms / unit) as u16,
                                steps,
                                1,
                                560.,
                                self.slider_change(8, cx),
                            ),
                        ))
                        .child(components::slider_field(
                            "End",
                            time(draft.end_ms),
                            560.,
                            components::slider(
                                9,
                                &self.sliders[9],
                                (draft.end_ms / unit) as u16,
                                steps,
                                1,
                                560.,
                                self.slider_change(9, cx),
                            ),
                        ))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    sound_button(
                                        "sound-draft-preview",
                                        if previewing { "Stop preview" } else { "Preview selection" },
                                        false,
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| this.preview_sound_draft(cx))),
                                )
                                .when(!self.sound_saving && valid, |line| {
                                    line.child(
                                        sound_button("sound-draft-save", "Add to soundboard", true).on_click(
                                            cx.listener(|this, _, window, cx| this.save_sound_draft(window, cx)),
                                        ),
                                    )
                                })
                                .when(self.sound_saving, |line| line.child(label("Saving sound…"))),
                        )
                        .child(label("The final clip is normalized, encoded at 96 kbps, and must fit inside 512 KiB."))
                } else {
                    settings_panel()
                        .child(row(
                            settings_row(
                                "Add a sound",
                                "Up to 25 MiB. Saved as a normalized 15-second clip at most.",
                            ),
                            if self.sound_loading {
                                div().child(label("Reading…"))
                            } else if used >= LIMIT {
                                div().child(label("Storage full"))
                            } else {
                                div().child(
                                    sound_button("sound-choose", "Choose audio", false)
                                        .on_click(cx.listener(|this, _, _, cx| this.choose_sound_file(cx))),
                                )
                            },
                        ))
                };
                let mut list = settings_panel();
                if mine.is_empty() {
                    list = list.child(label(match &self.soundboard_status {
                        crate::SoundboardStatus::Failed(message) => message.clone(),
                        crate::SoundboardStatus::Loaded => "No sounds yet.".to_string(),
                        _ => "Loading sounds…".to_string(),
                    }));
                }
                for (index, clip) in mine.iter().enumerate() {
                    let playing = self.playing_sound_id.as_deref() == Some(clip.id.as_str());
                    let renaming = self.renaming_sound.as_deref() == Some(clip.id.as_str());
                    let confirming = self.confirming_delete.as_deref() == Some(clip.id.as_str());
                    let (play_id, rename_id, delete_id) = (clip.id.clone(), clip.id.clone(), clip.id.clone());
                    let current_name = clip.name.clone();
                    let mut actions = div().flex().flex_shrink_0().items_center().gap(px(6.));
                    if renaming {
                        actions = actions.child(
                            small_button(("settings-sound-save", index), "Save").on_click(
                                cx.listener(|this, _, window, cx| this.save_sound_name(window, cx)),
                            ),
                        );
                    } else {
                        actions = actions
                            .child(
                                small_button(
                                    ("settings-sound-test", index),
                                    if playing { "Stop" } else { "Test" },
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.play_clip(play_id.clone(), cx);
                                })),
                            )
                            .child(
                                small_button(("settings-sound-rename", index), "Rename").on_click(
                                    cx.listener(move |this, _, window, cx| {
                                        this.renaming_sound = Some(rename_id.clone());
                                        this.confirming_delete = None;
                                        let name = current_name.clone();
                                        this.sound_name_input.update(cx, |state, cx| {
                                            state.set_value(name, window, cx);
                                            state.focus(window, cx);
                                        });
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                small_button(
                                    ("settings-sound-delete", index),
                                    if confirming { "Confirm delete" } else { "Delete" },
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if this.confirming_delete.as_deref() == Some(delete_id.as_str()) {
                                        this.delete_sound(delete_id.clone(), cx);
                                    } else {
                                        this.confirming_delete = Some(delete_id.clone());
                                        cx.notify();
                                    }
                                })),
                            );
                    }
                    list = list.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .map(|line| {
                                if renaming {
                                    line.child(
                                        material_surface(MATERIAL_DARK, 12., StrokeKind::Neutral)
                                            .flex_1()
                                            .h(px(36.))
                                            .px(px(4.))
                                            .flex()
                                            .items_center()
                                            .child(Input::new(&self.sound_name_input).appearance(false)),
                                    )
                                } else {
                                    line.child(settings_row(clip.name.clone(), clip.duration_label.clone()))
                                }
                            })
                            .child(actions),
                    );
                }
                div()
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .max_w(px(620.))
                    .child(settings_title(
                        "Soundboard",
                        "Test, rename or remove your sounds. Testing plays only for you.",
                    ))
                    .child(
                        settings_panel().child(components::slider_field(
                            "Your soundboard volume",
                            format!("{}%", self.state.preferences.soundboard_volume),
                            560.,
                            self.slider_for(2, self.state.preferences.soundboard_volume, 100, 5, 560., cx),
                        )),
                    )
                    .child(storage)
                    .child(add_panel)
                    .child(list)
            }
            SettingsTab::About => div()
                .flex()
                .flex_col()
                .gap(px(18.))
                .max_w(px(620.))
                .child(settings_title(
                    "About Nitro",
                    "A focused space for one-to-one chat, voice, and shared moments.",
                ))
                .child(
                    settings_panel()
                        .child(settings_row(format!("Nitro {}", env!("CARGO_PKG_VERSION")), update_text))
                        .child(settings_row(
                            "Signed in as",
                            format!("@{}", self.state.profile.username),
                        )),
                )
                .when_some(update_button, |column, button| column.child(button)),
            _ => div()
                .flex()
                .flex_col()
                .gap(px(16.))
                .max_w(px(620.))
                .child(
                    div()
                        .text_size(px(19.))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(0xf5f5f7))
                        .child("Chat"),
                )
                .child(
                    material_surface(MATERIAL_SUBTLE, 16., StrokeKind::Neutral)
                        .p(px(16.))
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_size(px(13.5))
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(rgb(0xf2f2f3))
                                                .child("Enter to send messages"),
                                        )
                                        .child(label(
                                            "Press Enter to send immediately, Shift+Enter for a new line.",
                                        )),
                                )
                                .child(
                                    material_interactive(
                                        "toggle-enter-send",
                                        if self.state.preferences.enter_to_send {
                                            MATERIAL_GREEN
                                        } else {
                                            MATERIAL_DARK
                                        },
                                        12.,
                                        StrokeKind::Colored,
                                    )
                                    .h(px(28.))
                                    .px(px(12.))
                                    .flex()
                                    .items_center()
                                    .text_size(px(12.))
                                    .text_color(rgb(0xffffff))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.state.preferences.enter_to_send =
                                            !this.state.preferences.enter_to_send;
                                        this.persist();
                                        cx.notify();
                                    }))
                                    .child(if self.state.preferences.enter_to_send {
                                        "Enabled"
                                    } else {
                                        "Disabled"
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_size(px(13.5))
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(rgb(0xf2f2f3))
                                                .child("Show rich media previews"),
                                        )
                                        .child(label(
                                            "Render inline WebP galleries and WebM video clips in chat.",
                                        )),
                                )
                                .child(
                                    material_interactive(
                                        "toggle-media-previews",
                                        if self.state.preferences.show_media_previews {
                                            MATERIAL_GREEN
                                        } else {
                                            MATERIAL_DARK
                                        },
                                        12.,
                                        StrokeKind::Colored,
                                    )
                                    .h(px(28.))
                                    .px(px(12.))
                                    .flex()
                                    .items_center()
                                    .text_size(px(12.))
                                    .text_color(rgb(0xffffff))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.state.preferences.show_media_previews =
                                            !this.state.preferences.show_media_previews;
                                        this.persist();
                                        cx.notify();
                                    }))
                                    .child(if self.state.preferences.show_media_previews {
                                        "Enabled"
                                    } else {
                                        "Disabled"
                                    }),
                                ),
                        ),
                ),
        };

        div()
            .size_full()
            .relative()
            .pt(px(96.))
            .flex()
            .child(
                div()
                    .id("settings-back-btn")
                    .cursor_pointer()
                    .absolute()
                    .top(px(28.))
                    .left(px(30.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(15.))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xb4b4be))
                    .hover(|s| s.text_color(rgb(0xffffff)))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.shell_view = ShellView::Chat;
                        cx.notify();
                    }))
                    .child(
                        svg()
                            .path("ChevronLeft")
                            .size(px(16.))
                            .text_color(rgb(0xb4b4be)),
                    )
                    .child("Settings"),
            )
            .child(sidebar_nav)
            .child(
                div()
                    .id("settings-panel-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(36.))
                    .pb(px(48.))
                    .child(panel_content),
            )
    }
}
