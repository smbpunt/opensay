# Development Plan

## Status
| Phase      | Current              | Progress |
|------------|----------------------|----------|
| **Active** | 5 - Cross-Platform   | 0%       |

## Phases

| # | Phase                  | Status      | Validation                                   |
|---|------------------------|-------------|----------------------------------------------|
| 1 | Setup & Infrastructure | Complete    | Build compile, PrivacyGuard bloque le réseau |
| 2 | Audio Capture          | Complete    | Enregistrement WAV fonctionnel, hot-plug OK  |
| 3 | Local Transcription    | Complete    | Transcription fichier référence réussie      |
| 4 | MVP macOS              | Complete    | Flux complet: voix → texte collé dans app    |
| 5 | **Cross-Platform & GPU** | Not Started | E2E sur 3 OS, benchmarks GPU               |
| 6 | Production Ready       | Not Started | Fresh install OK, audit privacy passé        |

---

## Phase 1: Setup & Infrastructure
- [x] Init Tauri v2 + React + Tailwind + Vite
- [x] Structure hexagonale (ports/adapters/domain)
- [x] PrivacyGuard (HTTP singleton)
- [x] ConfigStore + logging

## Phase 2: Audio Capture
- [x] AudioManager trait + cpal adapter
- [x] Ring buffer lock-free (60s)
- [x] Hot-plug state machine (Idle/Recording/DeviceLost/Recovering/Error)

## Phase 3: Local Transcription
- [x] Transcriber trait + WhisperCppTranscriber
- [x] ModelManager (download, SHA-256, catalog)
- [x] HardwareDetector (CPU)

## Phase 4: MVP macOS
- [x] VAD intégré (whisper.cpp params: no_speech_thold, entropy_thold, suppress_non_speech_tokens)
- [x] OutputManager (arboard + enigo) - ClipboardOutputManager adapter
- [x] Raccourci global Alt+Space (tauri-plugin-global-shortcut)
- [x] Zeroize audio post-transcription (AudioBuffer dropped after transcribe in toggle_recording)

## Phase 5: Cross-Platform & GPU
- [ ] Builds Windows/Linux
- [ ] Metal/CUDA/Vulkan acceleration
- [ ] Spécificités OS (Wayland, shortcuts, WASAPI)

## Phase 6: Production Ready
- [ ] Signing + Notarization
- [ ] Auto-updater (opt-in)
- [ ] Mode API cloud (opt-in)
- [ ] Onboarding

---

## Risques
| Risque            | Mitigation                    |
|-------------------|-------------------------------|
| CUDA CI sans GPU  | Libs pré-compilées            |
| Wayland shortcuts | CLI + IPC fallback            |
| Injection texte   | Retry + fallback char-by-char |
