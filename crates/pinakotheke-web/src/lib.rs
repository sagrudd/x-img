// SPDX-License-Identifier: MPL-2.0
//! Mnemosyne-compatible, host-integrable Yew application shell.

use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement, KeyboardEvent};
use yew::prelude::*;

#[derive(Clone, Copy)]
struct MediaCard {
    title: &'static str,
    source: &'static str,
    media_type: &'static str,
    alt_text: &'static str,
    object_state: &'static str,
    source_fragment: &'static str,
    playback_id: Option<&'static str>,
}

impl MediaCard {
    fn range_playable(self) -> bool {
        self.playback_id.is_some() && self.object_state == "Stored in ObjectStore"
    }

    fn playback_url(self) -> Option<String> {
        self.playback_id
            .filter(|_| self.range_playable())
            .map(|playback_id| format!("/api/playback/v1/{playback_id}"))
    }
}

const CARDS: [MediaCard; 6] = [
    MediaCard {
        title: "Aurora study",
        source: "X / SelectedArtist",
        media_type: "Image · PNG",
        alt_text: "Abstract aurora-coloured study in a square composition.",
        object_state: "Stored in ObjectStore",
        source_fragment: "source-record-aurora",
        playback_id: None,
    },
    MediaCard {
        title: "Tidal form",
        source: "Website / Example gallery",
        media_type: "Image · JPEG",
        alt_text: "Observed thumbnail of a tidal sculpture against a pale background.",
        object_state: "Previously observed",
        source_fragment: "source-record-tidal",
        playback_id: None,
    },
    MediaCard {
        title: "Glass archive",
        source: "X / SelectedArtist",
        media_type: "Image · WebP",
        alt_text: "Layered glass-like geometric forms.",
        object_state: "Object unavailable",
        source_fragment: "source-record-glass",
        playback_id: None,
    },
    MediaCard {
        title: "Night garden",
        source: "Website / Example gallery",
        media_type: "Image · JPEG",
        alt_text: "Dark garden study with a bright central bloom.",
        object_state: "Stored in ObjectStore",
        source_fragment: "source-record-garden",
        playback_id: None,
    },
    MediaCard {
        title: "Field record",
        source: "Website / Example gallery",
        media_type: "Video · normalized MP4",
        alt_text: "Short field recording with a muted abstract poster frame.",
        object_state: "Stored in ObjectStore",
        source_fragment: "source-record-field-record",
        playback_id: Some("normalized-video-1"),
    },
    MediaCard {
        title: "Open geometry",
        source: "X / SelectedArtist",
        media_type: "Image · PNG",
        alt_text: "Open geometric line work in a square composition.",
        object_state: "Stored in ObjectStore",
        source_fragment: "source-record-geometry",
        playback_id: None,
    },
];

fn focus_by_id(id: &str) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let Some(element) = document.get_element_by_id(id) else {
        return;
    };
    if let Ok(element) = element.dyn_into::<HtmlElement>() {
        let _ = element.focus();
    }
}

fn focus_preview_control(current: Option<String>, reverse: bool) {
    const CONTROLS: [&str; 3] = ["preview-close", "preview-view-mode", "preview-source-link"];
    let current = current
        .as_deref()
        .and_then(|id| CONTROLS.iter().position(|candidate| candidate == &id));
    let next = match (current, reverse) {
        (Some(0), true) | (None, true) => CONTROLS.len() - 1,
        (Some(index), true) => index - 1,
        (Some(index), false) if index + 1 < CONTROLS.len() => index + 1,
        _ => 0,
    };
    focus_by_id(CONTROLS[next]);
}

/// Minimal root component for host integration.
#[function_component(App)]
pub fn app() -> Html {
    let selected = use_state(|| "all".to_owned());
    let density = use_state(|| "compact".to_owned());
    let active_card = use_state(|| 0usize);
    let refresh_state = use_state(|| "Not started".to_owned());
    let refresh_detail = use_state(|| {
        "X / SelectedArtist — Pending; Instagram / SampleCreator — Pending".to_owned()
    });
    let preview_open = use_state(|| false);
    let preview_mode = use_state(|| "Fit to pane".to_owned());
    let preview_ref = use_node_ref();
    let review_notice = use_state(|| "3 items need review".to_owned());
    let object_view = use_state(|| true);
    let filter = use_state(String::new);
    let sources = [
        ("all", "All sources", "6"),
        ("x", "X accounts", "2"),
        ("instagram", "Instagram accounts", "3"),
        ("websites", "Websites", "1"),
    ];

    {
        let preview_ref = preview_ref.clone();
        use_effect_with(*preview_open, move |is_open| {
            if *is_open && let Some(dialog) = preview_ref.cast::<HtmlElement>() {
                let _ = dialog.focus();
            }
            || ()
        });
    }

    let selected_card = CARDS.get(*active_card).copied().unwrap_or(CARDS[0]);
    let filter_text = (*filter).to_ascii_lowercase();
    html! {
        <div class="mn-app-shell ximg-shell">
            <header class="ximg-shell__header" aria-label="x-img workspace">
                <a class="ximg-shell__product" href="#main">{ "x-img" }</a>
                <nav class="ximg-shell__nav" aria-label="Primary navigation">
                    <a aria-current="page" href="#library">{ "Library" }</a>
                    <a href="#accounts">{ "Accounts" }</a>
                </nav>
                <p class="ximg-shell__host">{ "Hosted by Monas" }</p>
            </header>
            <main id="main" class="mn-app-main ximg-shell__main" tabindex="-1">
                <p class="ximg-shell__eyebrow">{ "Media workspace" }</p>
                <h1>{ "x-img library" }</h1>
                <p>{ "Review committed media from configured sources." }</p>

                <section class="ximg-filters" aria-label="Browse metadata filters">
                    <label>
                        {"Search metadata "}
                        <input
                            value={(*filter).clone()}
                            oninput={{
                                let filter = filter.clone();
                                Callback::from(move |event: InputEvent| {
                                    filter.set(event.target_unchecked_into::<HtmlInputElement>().value())
                                })
                            }}
                            placeholder="Account, media type, date"
                        />
                    </label>
                    <p>{if filter.is_empty() { "Filters: all records" } else { "Filter active: matching synthetic records" }}</p>
                </section>

                <section class="ximg-refresh" aria-labelledby="refresh-title">
                    <h2 id="refresh-title">{ "Account refresh" }</h2>
                    <p>{ format!("Status: {}", *refresh_state) }</p>
                    <p>{ format!("Per-account progress: {}", *refresh_detail) }</p>
                    <button onclick={{
                        let refresh_state = refresh_state.clone();
                        let refresh_detail = refresh_detail.clone();
                        Callback::from(move |_| {
                            refresh_state.set("Complete · 2 accounts · 3 new items".to_owned());
                            refresh_detail.set("X / SelectedArtist — Complete (2 new); Instagram / SampleCreator — Complete (1 new)".to_owned())
                        })
                    }}>{ "Refresh accounts" }</button>
                    <button onclick={{
                        let refresh_state = refresh_state.clone();
                        let refresh_detail = refresh_detail.clone();
                        Callback::from(move |_| {
                            refresh_state.set("Partial failure · retry available".to_owned());
                            refresh_detail.set("X / SelectedArtist — Complete (2 new); Instagram / SampleCreator — Failed: retry available".to_owned())
                        })
                    }}>{ "Show retry state" }</button>
                    <button onclick={{
                        let refresh_state = refresh_state.clone();
                        Callback::from(move |_| refresh_state.set("Retry scheduled safely for Instagram / SampleCreator".to_owned()))
                    }}>{ "Retry failed account" }</button>
                </section>

                <section class="ximg-review" aria-labelledby="review-title">
                    <h2 id="review-title">{ "Review queue" }</h2>
                    <p>{ format!("State: {}", *review_notice) }</p>
                    <p>{ "Selected records: 0 · New · Reviewed · Hidden · Removed" }</p>
                    <button
                        aria-pressed={object_view.to_string()}
                        onclick={{
                            let object_view = object_view.clone();
                            Callback::from(move |_| object_view.set(!*object_view))
                        }}
                    >{if *object_view { "Showing: Stored in ObjectStore originals" } else { "Showing: Previously observed thumbnails" }}</button>
                    <p title="This label is a reversible overlay; stored bytes are never changed.">
                        {if *object_view { "● Stored in ObjectStore — committed original" } else { "◌ Previously observed — thumbnail only" }}
                    </p>
                    <button onclick={{
                        let review_notice = review_notice.clone();
                        Callback::from(move |_| review_notice.set("Batch reviewed · 3 items · Undo available".to_owned()))
                    }}>{ "Mark selected reviewed" }</button>
                    <button onclick={{
                        let review_notice = review_notice.clone();
                        Callback::from(move |_| review_notice.set("Batch hidden · 3 items · Undo available".to_owned()))
                    }}>{ "Hide selected" }</button>
                    <button onclick={{
                        let review_notice = review_notice.clone();
                        Callback::from(move |_| review_notice.set("Batch action undone".to_owned()))
                    }}>{ "Undo batch action" }</button>
                </section>

                <section class="ximg-source-nav" aria-labelledby="source-context">
                    <h2 id="source-context">{ "Sources" }</h2>
                    <p>{ format!("Selected context: {}", sources.iter().find(|source| source.0 == (*selected).as_str()).map(|source| source.1).unwrap_or("All sources")) }</p>
                    <ul>
                        { for sources.iter().map(|(id, label, count)| {
                            let selected = selected.clone();
                            let is_selected = *id == (*selected).as_str();
                            let id = (*id).to_owned();
                            html! {
                                <li>
                                    <button
                                        class={classes!("ximg-source-nav__item", is_selected.then_some("is-selected"))}
                                        aria-pressed={is_selected.to_string()}
                                        onclick={Callback::from(move |_| selected.set(id.clone()))}
                                    ><span>{ *label }</span><span>{ format!("{} sources", count) }</span></button>
                                </li>
                            }
                        }) }
                    </ul>
                </section>

                <section class="ximg-shell__empty" aria-labelledby="library-state">
                    <h2 id="library-state">{ "No committed media in this context" }</h2>
                    <p>{ "Counts describe configured sources; committed media appears here after verified admission." }</p>
                </section>

                <section class="ximg-gallery" aria-labelledby="gallery-title">
                    <div class="ximg-gallery__toolbar">
                        <h2 id="gallery-title">{ "Thumbnail browser" }</h2>
                        <button onclick={{
                            let density = density.clone();
                            Callback::from(move |_| density.set(if *density == "compact" { "comfortable".to_owned() } else { "compact".to_owned() }))
                        }}>{ format!("Density: {}", *density) }</button>
                    </div>
                    <p>{ "Synthetic metadata records; image pixels are never retained by the x-img Web client." }</p>
                    <div class={classes!("ximg-gallery__grid", format!("is-{}", *density))}>
                        { for CARDS.iter().enumerate().filter(|(_, card)| {
                            card.title.to_ascii_lowercase().contains(&filter_text)
                                || card.source.to_ascii_lowercase().contains(&filter_text)
                                || card.media_type.to_ascii_lowercase().contains(&filter_text)
                        }).map(|(index, card)| {
                            let active_card = active_card.clone();
                            let preview_open = preview_open.clone();
                            let preview_mode = preview_mode.clone();
                            let is_selected = index == *active_card;
                            html! {
                                <button
                                    id={format!("preview-trigger-{index}")}
                                    class={classes!("ximg-gallery__card", is_selected.then_some("is-selected"))}
                                    aria-haspopup="dialog"
                                    aria-pressed={is_selected.to_string()}
                                    onclick={Callback::from(move |_| {
                                        active_card.set(index);
                                        preview_mode.set("Fit to pane".to_owned());
                                        preview_open.set(true)
                                    })}
                                >
                                    <span class="ximg-gallery__placeholder" aria-hidden="true"></span>
                                    <span>{ card.title }</span>
                                    <small>{ format!("{} · {}", card.media_type, card.object_state) }</small>
                                </button>
                            }
                        }) }
                    </div>
                </section>

                { if *preview_open {
                    let close_preview_state = preview_open.clone();
                    let close_preview_card = active_card.clone();
                    let close_preview = Callback::from(move |_| {
                        close_preview_state.set(false);
                        focus_by_id(&format!("preview-trigger-{}", *close_preview_card));
                    });
                    let keyboard_preview_state = preview_open.clone();
                    let keyboard_active_card = active_card.clone();
                    let on_keydown = Callback::from(move |event: KeyboardEvent| match event.key().as_str() {
                        "Escape" => {
                            event.prevent_default();
                            keyboard_preview_state.set(false);
                            focus_by_id(&format!("preview-trigger-{}", *keyboard_active_card));
                        }
                        "Tab" => {
                            event.prevent_default();
                            let active_id = web_sys::window()
                                .and_then(|window| window.document())
                                .and_then(|document| document.active_element())
                                .and_then(|element| element.get_attribute("id"));
                            focus_preview_control(active_id, event.shift_key());
                        }
                        _ => {}
                    });
                    let preview_mode = preview_mode.clone();
                    let view_mode = (*preview_mode).clone();
                    let toggle_view = Callback::from(move |_| {
                        preview_mode.set(if *preview_mode == "Fit to pane" { "Original size".to_owned() } else { "Fit to pane".to_owned() })
                    });
                    html! {
                        <section
                            ref={preview_ref.clone()}
                            class="ximg-preview"
                            role="dialog"
                            aria-modal="true"
                            aria-labelledby="preview-title"
                            aria-describedby="preview-summary"
                            tabindex="-1"
                            onkeydown={on_keydown}
                        >
                            <div class="ximg-preview__pane">
                                <div class="ximg-preview__heading">
                                    <div><p class="ximg-shell__eyebrow">{ "Selected record" }</p><h2 id="preview-title">{ selected_card.title }</h2></div>
                                    <button id="preview-close" autofocus=true onclick={close_preview}>{ "Close preview" }</button>
                                </div>
                                <p id="preview-summary">{ format!("{} · {}", selected_card.media_type, selected_card.object_state) }</p>
                                <div class="ximg-preview__layout">
                                    <section class={classes!("ximg-preview__visual", (view_mode == "Original size").then_some("is-original"))} aria-label="Media visual">
                                        <div role="img" aria-label={selected_card.alt_text} class="ximg-preview__visual-proxy">
                                            <span>{ "Synthetic visual proxy" }</span>
                                            <small>{ selected_card.alt_text }</small>
                                        </div>
                                        <button id="preview-view-mode" aria-pressed={(view_mode == "Original size").to_string()} onclick={toggle_view}>
                                            { if view_mode == "Fit to pane" { "View original size" } else { "Fit to pane" } }
                                        </button>
                                    </section>
                                    <aside class="ximg-preview__details" aria-label="Selected media details">
                                        <dl>
                                            <div><dt>{ "Source" }</dt><dd>{ selected_card.source }</dd></div>
                                            <div><dt>{ "Media type" }</dt><dd>{ selected_card.media_type }</dd></div>
                                            <div><dt>{ "Object state" }</dt><dd>{ selected_card.object_state }</dd></div>
                                            <div><dt>{ "Alt text" }</dt><dd>{ selected_card.alt_text }</dd></div>
                                        </dl>
                                        <a id="preview-source-link" href={format!("#{}", selected_card.source_fragment)}>{ "View source metadata" }</a>
                                        { if let Some(playback_url) = selected_card.playback_url() {
                                            html! {
                                                <section class="ximg-preview__playback" aria-labelledby="playback-title">
                                                    <h3 id="playback-title">{ "Normalized video playback" }</h3>
                                                    <p>{ "Ready · authorized ObjectStore range delivery" }</p>
                                                    <video controls=true preload="metadata" src={playback_url} aria-label={format!("Play {}", selected_card.title)}>
                                                        { "Your browser cannot play the verified normalized video." }
                                                    </video>
                                                </section>
                                            }
                                        } else if selected_card.object_state == "Object unavailable" {
                                            html! {
                                                <section class="ximg-preview__unavailable" role="status" aria-live="polite">
                                                    <h3>{ "Object unavailable" }</h3>
                                                    <p>{ "The committed object cannot be read. Playback and original media are unavailable; x-img does not fall back to the source URL." }</p>
                                                </section>
                                            }
                                        } else {
                                            html! {
                                                <section class="ximg-preview__unavailable" role="status">
                                                    <h3>{ "No video playback for this record" }</h3>
                                                    <p>{ "This image record remains available through its authorized ObjectStore object state." }</p>
                                                </section>
                                            }
                                        }}
                                    </aside>
                                </div>
                            </div>
                        </section>
                    }
                } else { Html::default() } }
            </main>
            <footer class="mn-brand-footer" aria-label="Mnemosyne Biosciences provenance">
                <div class="mn-brand-footer__content">
                    <span class="mn-brand-footer__wordmark">{ "Mnemosyne Biosciences" }</span>
                    <span class="mn-brand-footer__product">{ "x-img · host-integrated workspace" }</span>
                </div>
                <span class="mn-brand-footer__mark" aria-hidden="true">{ "◒" }</span>
            </footer>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::CARDS;

    #[test]
    fn only_a_ready_normalized_object_receives_a_range_playback_url() {
        let video = CARDS
            .iter()
            .find(|card| card.playback_id.is_some())
            .unwrap();
        let unavailable = CARDS
            .iter()
            .find(|card| card.object_state == "Object unavailable")
            .unwrap();
        assert_eq!(
            video.playback_url().as_deref(),
            Some("/api/playback/v1/normalized-video-1")
        );
        assert_eq!(unavailable.playback_url(), None);
    }
}
