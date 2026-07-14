// SPDX-License-Identifier: MPL-2.0
//! Mnemosyne-compatible, host-integrable Yew application shell.

use yew::prelude::*;

/// Minimal root component for host integration.
#[function_component(App)]
pub fn app() -> Html {
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
                <section class="ximg-shell__empty" aria-labelledby="library-state">
                    <h2 id="library-state">{ "Library not yet connected" }</h2>
                    <p>{ "Configure a source and refresh it when platform policy permits." }</p>
                </section>
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
