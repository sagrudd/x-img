Release audits
==============

``scripts/audit/check.sh`` is the reproducible local authority for the x-img
privacy, security, accessibility, dependency, license, and version audit. It
uses the strict coverage matrix in
``docs/fixtures/release-audit-matrix.json`` and fails when a required category
or invariant disappears. Hosted CI may mirror this command but is not required.

.. code-block:: console

   scripts/audit/check.sh

Audit coverage
--------------

Privacy
   Scan tracked and untracked repository candidates for media/bioinformatics
   payload extensions and credential signatures. Existing strict fixture checks
   continue to reject secret fields and unredacted signed-query examples.

Security and Firefox permissions
   Reject unsafe Rust, dynamic JavaScript evaluation/HTML injection, a weakened
   extension CSP, required permissions outside the reviewed
   ``storage``, ``activeTab``, ``scripting``, and runtime ``permissions`` set,
   or non-optional/non-HTTPS site access. The extension still asks the user for
   each exact origin at runtime; the wildcard only declares what may be asked.

Accessibility
   Require semantic extension documents, labelled status regions, explicit
   button behavior, keyboard-visible Yew focus treatment, and the existing
   Yew navigation/dialog pressed/current semantics. These static gates
   complement, but do not replace, assistive-technology and WCAG acceptance in
   a packaged production build.

Licenses and dependencies
   Require the MPL-2.0 repository license and SPDX notices on source files, then
   run ``cargo-deny`` against the locked graph for allowed licenses, registry
   sources, wildcard policy, duplicates, and RustSec advisories.

Recorded dependency exceptions
------------------------------

``deny.toml`` contains two narrow, reasoned exceptions in the current Yew 0.23
transitive graph. ``proc-macro-error`` and ``bincode`` have unmaintained-status
advisories with no compatible maintained replacement at this layer; neither is
a reported memory-safety/security vulnerability, and x-img does not directly
use worker serialization. They remain visible policy entries to revisit with
the next Yew upgrade. Duplicate ``http``, ``syn``, and ``thiserror`` generations
are warnings caused by that same WebAssembly dependency graph, not silent
allowances.

Version audit
-------------

The audit checks the Firefox manifest at its actual
``firefox-extension/manifest.json`` location, along with Rust, Sphinx, and
planning mirrors. This closes the earlier blind spot where the generic quality
check searched obsolete extension directory patterns.
