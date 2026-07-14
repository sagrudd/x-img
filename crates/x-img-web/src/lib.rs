// SPDX-License-Identifier: MPL-2.0
//! Mnemosyne-compatible, host-integrable Yew application shell.

use yew::prelude::*;

/// Minimal root component for host integration.
#[function_component(App)]
pub fn app() -> Html {
    let selected = use_state(|| "all".to_owned());
    let sources = [
        ("all", "All sources", "6"),
        ("x", "X accounts", "2"),
        ("instagram", "Instagram accounts", "3"),
        ("websites", "Websites", "1"),
    ];
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
                <section class="ximg-source-nav" aria-labelledby="source-context">
                    <h2 id="source-context">{ "Sources" }</h2>
                    <p>{ format!("Selected context: {}", sources.iter().find(|source| source.0 == (*selected).as_str()).map(|source| source.1).unwrap_or("All sources")) }</p>
                    <ul>
                    { for sources.iter().map(|(id, label, count)| { let selected = selected.clone(); let is_selected = *id == (*selected).as_str(); let id = (*id).to_owned(); html! { <li><button class={classes!("ximg-source-nav__item", is_selected.then_some("is-selected"))} aria-pressed={is_selected.to_string()} onclick={Callback::from(move |_| selected.set(id.clone()))}><span>{ *label }</span><span>{ format!("{} sources", count) }</span></button></li> } }) }
                    </ul>
                </section>
                <section class="ximg-shell__empty" aria-labelledby="library-state"><h2 id="library-state">{ "No committed media in this context" }</h2><p>{ "Counts describe configured sources; committed media appears here after verified admission." }</p></section>
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
