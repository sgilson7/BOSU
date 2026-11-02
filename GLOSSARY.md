# Glossary

The source of truth for what each word in this tool means. Strings, tests, the plan and the README use a term in the sense given here. The page shows a definition when a reader hovers the term.

Write each entry as a `### Term` heading, then one or two plain sentences, then an optional `Not:` line listing words that must not be used for it. A number a rule decides is a `{placeholder}` the core fills, so the glossary cannot disagree with the rules. Keep entries in alphabetical order. `crates/core/tests/copy.rs` checks the format, the placeholders and the `Not:` words; `TONE.md` governs the prose.

<!--
An example entry. It sits inside this comment, so it does not count.

### Spout

Where slush comes out. Each line has up to {line.spouts} spouts.

Not: tap, dispenser, nozzle
-->

### Accuracy

How close a play came to all Perfect: the points the targets earned, before the combo, over the most they could earn, as a percentage.

### Approach ring

The ring that closes in on a target. It meets the target's edge at the moment to press.

Not: approach circle

### Audio offset

How far the game moves your presses to match what you hear, in milliseconds. Set it on the calibration screen.

### Chart

The targets of one song at one difficulty: when each comes, where it sits, and which ones are holds.

Not: beatmap, map

### Combo

How many targets, hold ticks and hold ends in a row you have hit without a Miss or a break. A longer combo multiplies the points of each hit, up to double.

### Difficulty

How hard a chart is: Easy, Normal, Hard, Expert or Master. Harder charts have more targets, smaller targets, a faster approach and tighter windows.

### Good

A hit inside the widest window, outside Great. It keeps the combo.

### Great

A hit inside the middle window, outside Perfect.

### Hold

A target with a path: press on it, keep the cursor on the moving point until the end, then let go. Each beat on the path is a tick.

Not: slider

### Miss

A target never pressed inside its window. It ends the combo and earns nothing.

### Perfect

A hit inside the narrowest window around the beat.

### Score

The points of every hit, tick and hold end, each multiplied by the combo at that moment.

### Target

A circle to press on its beat. Its number counts up within a phrase of the song.

Not: hit circle, note, hit object

### Visual offset

How far the game draws the targets ahead of or behind the music, in milliseconds, for a screen that shows a frame late.

