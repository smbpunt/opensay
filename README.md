# OpenSay

Application desktop de transcription vocale — Privacy-first, Cross-platform

## Fonctionnalités

- **100% local par défaut** : aucune donnée ne quitte la machine sans action explicite
- **Transcription via Whisper** : whisper.cpp avec modèles GGUF optimisés
- **Cross-platform natif** : macOS, Windows, Linux (pas un wrapper web)
- **Accélération GPU** : Metal (Apple Silicon), CUDA (NVIDIA), Vulkan (fallback)
- **Mode API cloud opt-in** : OpenAI, Deepgram, AssemblyAI, endpoint custom

## Stack technique

| Composant | Technologie                       |
|-----------|-----------------------------------|
| Backend   | Rust + Tauri v2                   |
| Frontend  | React + TypeScript + Tailwind CSS |
| Build     | Vite                              |
| Inférence | whisper.cpp (via whisper-rs)      |
| Audio     | cpal + Silero VAD (GGML natif)    |

## Architecture

L'application suit une architecture hexagonale (Ports & Adapters) permettant le hot-swap entre backends de transcription :

- **WhisperCpp** : inférence locale
- **OpenAI / Deepgram / Custom API** : cloud opt-in

Les données audio ne sont jamais écrites sur disque et sont zéroïsées après transcription.

## Installation

[Instructions à compléter]

## Développement

[Instructions à compléter]

## Licence

[À définir]
