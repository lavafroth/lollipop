# Lollipop

Opinionated key remapper that brings sticky keys functionality like Android's
AOSP keyboard to Linux.

## Core Logic

Initially a key is unlatched.

- Press a modifier once: Latched
  - Press the same modifier within 500ms: Locked
    - Press the same modifier: Unlatched
  - Press the same modifier beyond 500ms: Unlatched
