# Manual Testing Checklist

Automated tests (`cargo test --workspace`) cover pure logic and widget
*construction*. The items below need a human at a real display — GTK layout,
animations, and hardware-dependent flows (microphone, TTS engines) aren't
practical to assert on automatically.

## App shell
- [ ] App launches to the Vocabulary tab with no crash on first run (empty DB)
- [ ] Language indicator in the header shows the configured active language
- [ ] All five tabs (Vocabulary, Kanji, Review, Pronunciation, Statistics) are reachable via the ViewSwitcher
- [ ] Kanji tab is hidden when the active language is Spanish
- [ ] App menu → About shows the About window; → Quit closes the app
- [ ] Resizing the window below ~600px wide collapses the header into the narrow layout

## Vocabulary tab
- [ ] Sections group correctly by level (JLPT/CEFR)
- [ ] "Show All" expands a section into a grid and collapses back cleanly
- [ ] Clicking a card would open its detail dialog (once wired to real click handlers)

## Kanji tab
- [ ] Same section/"Show All" behavior as Vocabulary
- [ ] Large character rendering is legible at the default window size

## Review tab
- [ ] "Show Answer" reveals the back of the card and the four rating buttons
- [ ] Rating a card advances to the next one and updates the progress label
- [ ] Rating the last card shows "Review complete"
- [ ] Ratings persist: quit and relaunch, confirm the reviewed card's next-due date changed

## Pronunciation tab
- [ ] With a VOICEVOX Engine running locally and a Piper model configured: Play produces audible reference audio
- [ ] Record captures from the microphone and the waveform updates
- [ ] Score and feedback text appear after recording
- [ ] Without any backend configured, Play/Record show a clear error toast instead of crashing

## Statistics tab
- [ ] Summary tiles (reviews, correct, retention, streak) reflect actual review history after a few review sessions
- [ ] Review history bar chart renders proportional bars
- [ ] Deck list shows due/total counts that match the Review tab's queue

## Preferences
- [ ] Changing the active language persists across restart
- [ ] Theme switch (System/Light/Dark) takes effect immediately
- [ ] Clear Cache button actually empties the audio cache directory
- [ ] Audio device dropdowns list real system devices

## Startup diagnostics
- [ ] Missing dictionary file shows a toast naming the expected path
- [ ] No microphone / no output device shows the corresponding toast
- [ ] Toasts are dismissible and don't block interaction with the rest of the app
