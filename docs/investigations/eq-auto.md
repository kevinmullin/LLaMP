# EQ AUTO

Written 2026-09-14 from published behavior notes. Not from the Winamp source drop.

## What the notes agree on

The Winamp Lite 2.72 user’s manual, under Equalizer → Auto, says the button automatically loads any specific auto-load presets associated with the selected track. Presets → Save → Auto-load Preset associates the current preamp and band settings with the track that is playing, after prompting for a name. That preset loads when that track plays, if Auto is clicked.

The same manual’s On button puts equalizer changes into effect. If On is not clicked, the track plays without those changes.

The Winamp Equalizer Q&A (nunzioweb, revised 5 May 2005) asks what the Auto button does and answers that auto presets are set for specific media files and load for each file when the Equalizer and Auto are both on. It says auto-presets are based on filenames. A file with no auto preset does not reset unless a Default preset has been saved (Presets → Save → Default). The same page says the equalizer is not an auto-leveler; auto presets plus the preamp slider are how a person fakes that by hand.

The Winamp product page for the equalizer describes the same sequence: On enables the graph, Auto loads a per-track preset when both are on, and a saved Default is the fallback for a file that has no auto preset.

Those three notes are the same behavior. AUTO is not an automatic preamp.

## What was not used

A Hydrogenaudio reply remembers that unchecking Auto means you must set the equalizer on every file. That is a recollection, not a gain law, and it matches filename presets.

Winamp 5.51’s `eq_limiter` ini flag is a later clipping limiter for the 4Front equalizer. It is not the AUTO button. This note does not turn that flag into a preamp guess.

The Q&A names `Winamp.q1`, `Winamp.q2`, and `.eqf` as the files Winamp uses. It does not publish their layout. We do not parse them.

## What the button does

- On bypasses the cascade when it is off. On applies the slider targets. Off is the default.
- Auto, while On is also on, loads the auto-load preset whose filename matches the playing track. If there is none, it loads Default when one has been saved. If neither exists, the sliders stay where they are.
- The association key is the filename, not a full path. That is the published rule, including the note that CD tracks named `Track##.cda` share a preset.
- A preset load writes targets. The callback still slews. It does not write coefficients from the UI thread.
- Auto does not compute a preamp from the band boosts.

The accessibility label is “Auto-load preset for this track.”
