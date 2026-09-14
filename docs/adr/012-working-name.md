# ADR 012 — Working name LLaMP, rename gate before public identity

- Status: accepted
- Date: 2026-09-13
- Amended: 2026-09-13. Named collisions checked. Still not a trademark opinion. The working-name path is Application Support, not a copied music folder.

## Context

The product needs a name that is not Winamp. Winamp is a trademark. The only acceptable use is descriptive: “compatible with classic Winamp skins.”

The owner chose the working name LLaMP, “Large Llama Music Player,” and supplied a pixel mark of a llama in headphones. That mark is the visual identity. The letters are a separate legal question.

Collisions, not a formal clearance:

- [Llammp](https://github.com/yoanbernabeu/llammp) (MIT, 2026) is a macOS Winamp-skin client that drives Apple Music. Same product neighborhood, overlapping animal joke.
- Winamp’s owner rebranded from Llama Group to Winamp Group in 2025. A Winamp-skin player with “Llama” in the name sits next to that history.
- “LLaMP” is also an academic materials-science project (Lawrence Berkeley).
- Meta’s Llama mark is crowded. We do not ship a Meta model and must not look like we do.
- A USPTO registration for “LLAMP” (lighting) was cancelled in 2020. A cancelled registration is not a clearance.

No lawyer has cleared the name. This ADR is not a trademark opinion.

Checked 2026-09-13, still not a clearance:

- [Llammp](https://github.com/yoanbernabeu/llammp) exists. MIT. macOS 14.2+. It drives Apple Music. It does not decode.
- Llama Group’s extraordinary general meeting on 2025-10-20 renamed the company Winamp Group SA. Euronext updated the name and ticker to ALWIN on 2025-10-23.
- LBNL / UC Berkeley LLaMP is a materials-science RAG project (`chiang-yuan/llamp`, arXiv 2401.17244). Different product. Same letter sequence.
- USPTO serial 79089087, word mark LLAMP, lighting, registration 4095057, cancelled 2020-04-26. A cancelled registration is not a clearance.

## Decision

Documents, crate names, and the Application Support directory use LLaMP as the working name until the owner explicitly replaces it. Music files are not copied into a folder named for the app.

Before any of the following, the owner records a pass or a new name in this ADR’s status:

- Apple Developer bundle id registration
- Notarization
- A public GitHub repository name
- A website or a Sparkle appcast URL that embeds the name

The pixel mark is not part of the letter risk. It stays if the name changes. It is not redrawn to match a new name except for text that sits beside it.

Crate names (`llamp-core` and the rest) may keep the working prefix through 1.0 even if the display name changes, to avoid a rename cascade. The display name is a string in one place in the shell, not a search-and-replace across shaders.

If the owner wants a direction and has not hired counsel: keep the llama joke, drop the letter sequences that collide (`Llammp`, `LLaMP`, `Llama` as the product word). Do not silently rename.

## Consequences

- Phase 0 can scaffold crates with the `llamp-` prefix.
- Phase 12 cannot notarize until this gate is closed. That is an exit condition, not a suggestion.
- [LEGAL.md](../LEGAL.md) repeats the descriptive-use rule for the Winamp mark so a session that never opens this ADR still sees it.

## Alternatives

- **Ship the name and treat collisions as marketing.** Rejected. Llammp is a live project in the same niche. Registering a bundle id on top of that is the expensive-to-reverse step.
- **Pick a new name in this ADR without the owner.** Rejected. The owner chose the working name. The gate is theirs to close.
- **Avoid “llama” in the mark as well.** Rejected. The owner supplied the mark. The letters are the gate. The sprite stays.
