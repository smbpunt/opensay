# Development Plan

## Status
| Phase      | Current            | Progress |
|------------|--------------------|----------|
| **Active** | 4 - MVP macOS      | 0%       |

## Phases

| # | Phase                  | Status      | Validation                                   |
|---|------------------------|-------------|----------------------------------------------|
| 1 | Setup & Infrastructure | Complete    | Build compile, PrivacyGuard bloque le réseau |
| 2 | Audio Capture          | Complete    | Enregistrement WAV fonctionnel, hot-plug OK  |
| 3 | Local Transcription    | Complete    | Transcription fichier référence réussie      |
| 4 | **MVP macOS**          | Not Started | Flux complet: voix → texte collé dans app    |
| 5 | Cross-Platform & GPU   | Not Started | E2E sur 3 OS, benchmarks GPU                 |
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
- [ ] Silero VAD intégré
- [ ] OutputManager (arboard + enigo)
- [ ] Raccourci global + tray
- [ ] Zeroize audio post-transcription

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
