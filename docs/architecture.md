# OpenSay — Architecture Technique

**Application Desktop de Transcription Vocale · Privacy-First · Cross-Platform**

Version 1.1 — Février 2026 · Classification : Confidentiel

---

## 1. Vision et principes directeurs

OpenSay est une application desktop de transcription vocale conçue comme alternative privacy-first et cross-platform à Superwhisper. L'architecture repose sur quatre principes non négociables :

- **100% local par défaut** : aucune connexion réseau ne quitte la machine sans action explicite de l'utilisateur. L'architecture rend structurellement impossible toute fuite de données en mode local.
- **Cross-platform natif** : support équivalent sur macOS, Windows et Linux. Pas un wrapper web, mais une application utilisant les APIs natives de chaque OS.
- **Performance native** : inférence locale optimisée via accélération matérielle (Metal, CUDA, Vulkan) avec fallback CPU SIMD.
- **Modularité** : architecture hexagonale permettant le hot-swapping entre backends de transcription sans impact sur le reste du système.

---

## 2. Stack technique retenue

| Composant            | Technologie                                 | Version          |
|----------------------|---------------------------------------------|------------------|
| Framework applicatif | **Tauri v2**                                | 2.x stable       |
| Langage backend      | **Rust**                                    | Edition 2021+    |
| Frontend UI          | **React + TypeScript**                      | React 18+, TS 5+ |
| CSS                  | **Tailwind CSS**                            | 3.x              |
| Build frontend       | **Vite**                                    | 5.x              |
| Runtime d'inférence  | **whisper.cpp** (via whisper-rs)            | 0.15+            |
| Format de modèles    | **GGUF**                                    | —                |
| Capture audio        | **cpal**                                    | 0.15+            |
| VAD                  | **Silero VAD** (GGML natif via whisper.cpp) | v6.2.0           |

**Pourquoi cette stack :** Tauri v2 avec backend Rust donne un binaire d'environ 15–25 Mo (contre 150+ Mo pour Electron), une consommation RAM inférieure à 200 Mo, et un accès FFI direct aux bibliothèques C/C++ (whisper.cpp, cpal). Le modèle de sécurité capabilities-based de Tauri s'aligne avec l'approche privacy-first. Rust apporte la sécurité mémoire sans garbage collector, indispensable pour le traitement audio temps réel et la gestion sûre des buffers.

---

## 3. Architecture système

### 3.1 Pattern architectural : Hexagonal (Ports & Adapters)

L'application suit une architecture hexagonale. La logique métier (domaine de transcription) est isolée des détails d'implémentation (backend ML local, API cloud, capture audio, injection texte) via des traits Rust qui agissent comme des ports. Chaque implémentation concrète est un adaptateur interchangeable.

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Frontend React (WebView)                     │
│                   UI · Réglages · Gestion Modèles                   │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ Tauri IPC (Commands + Events)
┌──────────────────────────────▼──────────────────────────────────────┐
│                        AppController (Rust)                         │
│              Orchestration · État global · Cycle de vie              │
│                                                                     │
│  ┌─────────────┐  ┌──────────────────┐  ┌────────────────────────┐  │
│  │ AudioManager│  │TranscriptionEngine│  │    OutputManager       │  │
│  │   (Port)    │  │     (Port)        │  │      (Port)            │  │
│  │             │  │                   │  │                        │  │
│  │ Adaptateur: │  │ Adaptateurs:      │  │ Adaptateurs:           │  │
│  │  · cpal     │  │  · WhisperCpp     │  │  · macOS (CGEvent)     │  │
│  │             │  │  · OnnxBackend    │  │  · Windows (SendInput) │  │
│  │             │  │  · OpenAI API     │  │  · Linux (xdotool/     │  │
│  │             │  │  · Deepgram API   │  │          ydotool)      │  │
│  │             │  │  · Custom endpoint│  │                        │  │
│  └─────────────┘  └──────────────────┘  └────────────────────────┘  │
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────────┐    │
│  │ ModelManager │  │ PrivacyGuard │  │     ConfigStore         │    │
│  │ Download,    │  │ Firewall app,│  │ Préférences, état,      │    │
│  │ intégrité,   │  │ audit trail, │  │ consentement API        │    │
│  │ versioning   │  │ zéroisation  │  │                         │    │
│  └──────────────┘  └──────────────┘  └─────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 Flux principal

```
Raccourci clavier global
    │
    ▼
AudioManager (cpal) ──► Ring Buffer lock-free (60s)
    │                         │
    │                    Silero VAD
    │                    (filtrage silence)
    │                         │
    ▼                         ▼
Arrêt enregistrement ──► Segments audio PCM 16-bit 16kHz
                              │
                    TranscriptionEngine
                     ┌────────┴────────┐
                     │                 │
              Mode Local          Mode API (opt-in)
              WhisperCpp          OpenAI / Deepgram /
              (whisper-rs)        Custom endpoint
                     │                 │
                     └────────┬────────┘
                              │
                         Texte transcrit
                              │
                    OutputManager
                     ┌────────┴────────┐
                     │                 │
              arboard (clipboard)  +  enigo (paste simulé)
                              │
                     Application active reçoit le texte
```

### 3.3 Trait d'abstraction pour la transcription

```rust
/// Port principal — tout backend de transcription implémente ce trait.
#[async_trait]
pub trait Transcriber: Send + Sync {
    /// Transcrit un buffer audio et retourne le texte.
    async fn transcribe(
        &self,
        audio: &AudioBuffer,
        config: &TranscribeConfig,
    ) -> Result<TranscriptionResult, TranscribeError>;

    /// Déclare les capacités du backend (langues, streaming, réseau requis).
    fn capabilities(&self) -> BackendCapabilities;

    /// Vérifie si le backend est opérationnel (modèle chargé, API accessible).
    fn is_available(&self) -> bool;
}

/// Implémentations concrètes (adaptateurs) :
/// - WhisperCppTranscriber : inférence locale via whisper-rs
/// - OpenAiApiTranscriber : API cloud OpenAI (opt-in)
/// - DeepgramTranscriber  : API cloud Deepgram (opt-in)
/// - CustomApiTranscriber : endpoint compatible OpenAI API (auto-hébergé)
```

Le `TranscriptionEngine` utilise un pattern Strategy pour déléguer au backend actif. Le changement de backend (hot-swap) s'effectue à chaud via l'UI sans redémarrage.

---

## 4. Gestion audio cross-platform

### 4.1 Capture audio : cpal

La crate **cpal** fournit une API unifiée au-dessus des backends natifs : CoreAudio (macOS), WASAPI (Windows), ALSA (Linux, avec compatibilité PipeWire via alsa-compat). La capture s'effectue en mode callback non-bloquant.

**Format cible :** PCM 16-bit mono, 16 kHz — format natif attendu par Whisper. La conversion depuis le format du périphérique (généralement Float32 48 kHz stéréo) est effectuée en temps réel dans le callback cpal. Capturer directement en 16 kHz évite un downsampling coûteux en CPU.

**Buffering :** ring buffer lock-free (crate `ringbuf`) de 60 secondes par défaut, dimensionné selon la mémoire disponible et configurable.

**Résilience aux changements de périphérique (hot-plug) :** L'`AudioManager` implémente une machine à état avec auto-recovery. Si l'utilisateur débranche son micro, connecte des AirPods, ou change de périphérique par défaut en cours de session, le stream audio peut se fermer silencieusement ou panic. L'`AudioManager` détecte cette rupture et tente automatiquement de relancer la capture sur le périphérique par défaut après un court délai (500ms, puis backoff exponentiel). Si le rétablissement échoue après 3 tentatives, l'UI est notifiée pour informer l'utilisateur. Les états gérés sont : `Idle`, `Recording`, `DeviceLost`, `Recovering`, `Error`.

### 4.2 Voice Activity Detection (VAD)

Silero VAD est intégré en amont de la transcription pour filtrer les segments de silence et ne soumettre que la parole effective au modèle. Sans VAD, Whisper hallucine fréquemment sur les silences (répétition de "Thank you", charabia). Le VAD réduit significativement la charge CPU/GPU et améliore la qualité de transcription.

**Implémentation retenue :** whisper.cpp intègre nativement Silero VAD en format GGML depuis 2025. Le modèle VAD est converti du format ONNX original vers GGML (~864 Ko) et exécuté directement par le moteur d'inférence whisper.cpp, sans dépendance externe. Les bindings `whisper-rs` (v0.15+) exposent `WhisperVadContext`, `WhisperVadParams`, et `WhisperVadSegments` pour un contrôle fin (seuil de détection, durée minimale de parole/silence, padding).

**Avantage architectural majeur :** cette approche élimine la dépendance à ONNX Runtime (`ort`), qui aurait ajouté une complexité de linking significative sur les trois OS et un poids binaire non négligeable. Le VAD et la transcription partagent le même moteur d'inférence GGML, simplifiant drastiquement le build et la distribution.

### 4.3 Permissions et spécificités par OS

**macOS :** entrée `NSMicrophoneUsageDescription` obligatoire dans `Info.plist`. L'OS affiche un prompt de permission au premier accès micro. App Sandbox optionnel hors App Store.

**Windows :** gestion transparente via WASAPI. Attention au changement de périphérique par défaut en cours de session — l'AudioManager écoute les événements de changement de device et reconfigure le flux automatiquement.

**Linux :** cpal gère ALSA. Pour la compatibilité avec les distributions modernes (Fedora 39+, Ubuntu 23.10+), l'application doit fonctionner correctement sur la couche PipeWire (via la compatibilité ALSA de PipeWire). Tests de validation obligatoires sur PipeWire, PulseAudio et ALSA brut. Pour Flatpak/Snap : accès micro via le portail `xdg-desktop-portal`.

### 4.4 Mode streaming (optionnel)

Un mode streaming par segments de 5 secondes avec overlap de 0.5s permet la transcription quasi-temps-réel, complété par une passe de consolidation finale pour corriger les artefacts aux frontières de segments. Ce mode est activable dans les réglages et désactivé par défaut (le mode post-traitement complet donne de meilleurs résultats).

---

## 5. Raccourcis clavier globaux

### 5.1 Implémentation

Plugin officiel **`tauri-plugin-global-shortcut`** (v2), qui abstrait les mécanismes natifs : CGEvent (macOS), RegisterHotKey (Windows), XGrabKey/libxkbcommon (Linux X11).

Deux modes sont supportés :
- **Toggle** (par défaut) : une pression démarre l'enregistrement, une seconde l'arrête et déclenche la transcription.
- **Push-to-talk** : maintenir la touche pour enregistrer, relâcher pour transcrire.

Le raccourci par défaut est configurable par l'utilisateur via l'UI.

### 5.2 Limitation critique : Wayland (Linux)

Sous Wayland, les raccourcis clavier globaux sont restreints par design pour des raisons de sécurité. L'application ne peut pas capturer de touches globalement sans passer par les portails XDG (`org.freedesktop.portal.GlobalShortcuts`), qui ne sont pas encore universellement supportés par tous les compositeurs.

**Stratégie de contournement :**
- Détection automatique X11 vs Wayland au démarrage.
- Sous X11 : raccourcis globaux standard (XGrabKey).
- Sous Wayland : tentative via le portail XDG GlobalShortcuts. En cas d'échec, guide dans l'UI pour que l'utilisateur configure un raccourci au niveau de son compositeur (GNOME Settings / KDE Shortcuts) qui exécute une commande CLI de l'application (`OpenSay --toggle-recording`). L'application expose un socket IPC local pour recevoir ces commandes.

---

## 6. Injection de texte (Output)

L'insertion du texte transcrit dans l'application active suit un flux séquencé avec gestion des race conditions :

1. **Sauvegarde** du contenu actuel du presse-papier (pour restauration ultérieure).
2. **Écriture** du texte transcrit dans le presse-papier via le crate `arboard` (abstraction cross-platform : NSPasteboard sur macOS, Win32 Clipboard sur Windows, X11/Wayland clipboard sur Linux).
3. **Délai de synchronisation** de 50–100ms pour garantir que le presse-papier est effectivement mis à jour avant la simulation de collage. Ce délai est critique : sans lui, le `Cmd+V` / `Ctrl+V` peut coller l'ancien contenu si l'OS n'a pas encore finalisé l'écriture.
4. **Simulation de collage** via le crate `enigo` : `Cmd+V` sur macOS, `Ctrl+V` sur Windows/Linux.
5. **Restauration** du contenu précédent du presse-papier après un délai de 300ms.

**Spécificité Linux (X11) :** le presse-papier X11 fonctionne par négociation asynchrone entre fenêtres. L'`OutputManager` écrit dans les deux sélections (`CLIPBOARD` et `PRIMARY`) pour couvrir tous les cas d'usage. Sous Wayland, le protocole `wl_data_device` est utilisé via `arboard`.

**Fallback :** si la simulation de collage est bloquée par l'application cible (certains terminaux, applications sandboxées), un mode de frappe caractère par caractère est disponible via `enigo`. Ce mode est plus lent mais universel.

**Mécanisme de retry :** en cas d'échec détecté (focus window perdu, exception enigo), l'`OutputManager` effectue jusqu'à 2 tentatives supplémentaires avec un délai croissant (100ms, 200ms).

---

## 7. Système de gestion des modèles

### 7.1 Stratégie : download-on-demand

Aucun modèle n'est bundlé dans le binaire distribué. Au premier lancement, un assistant d'onboarding guide l'utilisateur pour télécharger un modèle adapté à son hardware. Cette approche maintient le téléchargement initial sous 25 Mo.

### 7.2 Modèle recommandé par défaut

**Whisper small en quantization Q5_1** (environ 150 Mo). Ce modèle offre le meilleur compromis qualité/vitesse pour la majorité des configurations desktop. Pour les machines modestes (CPU uniquement, < 8 Go RAM), le modèle **base Q5_1** (environ 52 Mo) est recommandé automatiquement par le HardwareDetector.

### 7.3 Catalogue des modèles

**Modèles locaux (GGUF via whisper.cpp) :**

| Modèle                  | Taille Q5_1 | Cas d'usage                               | Hardware minimum              |
|-------------------------|-------------|-------------------------------------------|-------------------------------|
| Whisper tiny            | ~52 Mo      | Tests, machines très limitées             | 2 Go RAM, CPU quelconque      |
| Whisper base            | ~80 Mo      | Usage basique, langues principales        | 4 Go RAM, CPU récent          |
| Whisper small           | ~150 Mo     | **Recommandé par défaut**, bon compromis  | 8 Go RAM, CPU multicœur       |
| Whisper medium          | ~580 Mo     | Qualité supérieure, multilingue           | 8 Go RAM, GPU ou CPU puissant |
| Whisper large-v3        | ~1280 Mo    | Qualité maximale                          | 16 Go RAM, GPU dédié          |
| Whisper large-v3-turbo  | ~580 Mo     | Qualité large, vitesse medium             | 8 Go RAM, GPU recommandé      |
| Distil-Whisper large-v3 | ~580 Mo     | ~2x plus rapide que large, qualité proche | 8 Go RAM, GPU recommandé      |

**APIs cloud (opt-in, clé API utilisateur requise) :**

| Service                                 | Particularité                        |
|-----------------------------------------|--------------------------------------|
| OpenAI Whisper API                      | Référence qualité, pricing à l'usage |
| Deepgram                                | Rapide, support streaming natif      |
| AssemblyAI                              | Diarisation, analyse avancée         |
| Endpoint custom (compatible OpenAI API) | Auto-hébergement entreprise          |

### 7.4 Niveaux de quantization disponibles

| Quantization | Ratio taille/qualité              | Recommandé pour                           |
|--------------|-----------------------------------|-------------------------------------------|
| Q4_0         | Très compact, qualité acceptable  | CPU faibles, RAM limitée (< 8 Go)         |
| **Q5_1**     | **Compact, très bonne qualité**   | **Usage par défaut — meilleur compromis** |
| Q8_0         | Plus lourd, excellente qualité    | GPU avec VRAM suffisante                  |
| F16          | Taille maximale, qualité maximale | GPU haut de gamme uniquement              |

### 7.5 Stockage des modèles

| OS      | Chemin par défaut                                           | Override             |
|---------|-------------------------------------------------------------|----------------------|
| macOS   | `~/Library/Application Support/OpenSay/models/`             | `OpenSay_MODELS_DIR` |
| Windows | `%LOCALAPPDATA%\OpenSay\models\`                            | `OpenSay_MODELS_DIR` |
| Linux   | `$XDG_DATA_HOME/OpenSay/models/` (défaut `~/.local/share/`) | `OpenSay_MODELS_DIR` |

### 7.6 ModelManager

Le ModelManager gère le cycle de vie complet des modèles :

- **Catalogue** : manifest JSON (embarqué + mise à jour optionnelle depuis CDN) avec métadonnées riches (taille, langues, hardware recommandé, benchmarks, licence, hash SHA-256).
- **Téléchargement** : via `reqwest`, avec reprise sur interruption (HTTP Range requests), barre de progression dans l'UI.
- **Intégrité** : vérification SHA-256 après téléchargement. Rejet et re-téléchargement automatique en cas de corruption.
- **Versioning** : migration automatique lors des mises à jour de format.
- **Suppression** : nettoyage propre via l'UI, avec confirmation.

---

## 8. Détection automatique du hardware

Le module `HardwareDetector` s'exécute au premier lancement et à chaque démarrage. Il produit un `HardwareProfile` utilisé pour sélectionner automatiquement la variante de modèle et le backend d'accélération optimaux.

### 8.1 Accélération matérielle par plateforme

| Plateforme          | Accélérateur utilisé                      | Fallback          | Méthode de détection                     |
|---------------------|-------------------------------------------|-------------------|------------------------------------------|
| macOS Apple Silicon | Metal (via whisper.cpp `WHISPER_METAL=1`) | CPU NEON          | IOKit + sysctl, détection Metal via objc |
| macOS Intel         | CPU AVX2/SSE                              | CPU basique       | `sysctl hw.optional.avx2_0`              |
| Windows NVIDIA      | CUDA (via `WHISPER_CUBLAS=1`)             | Vulkan → CPU AVX2 | Présence nvidia-smi, CUDA toolkit        |
| Windows AMD/Intel   | Vulkan                                    | CPU AVX2          | vulkaninfo, WMI                          |
| Linux NVIDIA        | CUDA                                      | Vulkan → CPU      | nvidia-smi, `/proc/driver/nvidia`        |
| Linux AMD           | Vulkan (ROCm optionnel)                   | CPU               | vulkaninfo, `rocm-smi`                   |
| Linux Intel         | Vulkan                                    | CPU               | vulkaninfo, `lspci`                      |

### 8.2 Recommandation automatique de modèle

Le HardwareDetector croise les capacités détectées avec les exigences des modèles :

- **GPU dédié + ≥ 8 Go VRAM** → Whisper large-v3-turbo Q8_0
- **GPU dédié + < 8 Go VRAM** → Whisper small Q8_0
- **Apple Silicon (M1+)** → Whisper small Q5_1 (Metal)
- **CPU puissant (≥ 8 cœurs, AVX2)** → Whisper small Q5_1
- **CPU modeste** → Whisper base Q5_1
- **Machine limitée (< 8 Go RAM)** → Whisper base Q4_0

L'utilisateur peut toujours override cette recommandation dans les réglages.

---

## 9. Architecture de confidentialité

### 9.1 PrivacyGuard : firewall applicatif

Le module `PrivacyGuard` agit comme un firewall interne au niveau du code Rust. En mode local (défaut), toute tentative d'appel réseau (DNS, HTTP, WebSocket) est bloquée avant même d'atteindre la couche OS. Seuls les appels explicitement autorisés par l'utilisateur (téléchargement de modèle, vérification de mise à jour, appel API cloud) passent cette barrière.

### 9.2 Audio éphémère et zéroisation mémoire

Les données audio capturées ne sont jamais écrites sur disque. Après transcription, le buffer mémoire est zéroïsé via le crate `zeroize` (écriture de zéros garantie non-optimisable par le compilateur). Aucun fichier temporaire audio n'est créé à aucun moment du pipeline.

Un mode historique optionnel peut sauvegarder les transcriptions **texte uniquement** (jamais l'audio) avec chiffrement local AES-256-GCM.

### 9.3 Consentement granulaire pour le mode API

L'activation du mode cloud nécessite trois actions distinctes et irréversibles par session :
1. Activer le mode API dans les réglages.
2. Configurer la clé API (stockée dans le keyring natif de l'OS via le crate `keyring` — Keychain sur macOS, Credential Manager sur Windows, libsecret sur Linux — jamais en clair dans un fichier).
3. Confirmer via une modale d'avertissement qui nomme le service, décrit les données envoyées, et requiert une action explicite.

### 9.4 Indicateurs visuels permanents

L'UI affiche en permanence le mode actif dans le tray et la fenêtre :
- **Mode local** : cadenas vert fermé, label "Local".
- **Mode API** : indicateur orange avec icône cloud et nom du service affiché.

### 9.5 Audit trail local

Un journal d'audit local (jamais envoyé, fichier rotatif) enregistre :
- Chaque session de transcription : horodatage, durée, modèle utilisé, mode (local/API).
- Chaque connexion réseau : destination, volume de données, raison.
- Chaque changement de configuration lié à la confidentialité.

Ce journal est consultable dans l'UI et exportable en JSON.

### 9.6 Zéro télémétrie

Aucune télémétrie, analytics, crash reporting, ou vérification automatique de mise à jour. Les logs d'erreur sont écrits localement dans un fichier rotatif et ne quittent jamais la machine.

---

## 10. Distribution et packaging

### 10.1 Packaging par plateforme

**macOS :**
- Format : `.dmg` contenant le `.app`.
- Deux binaires séparés : un pour Apple Silicon (arm64) avec accélération Metal, un pour Intel (x86_64) avec optimisations AVX2. Cela évite un Universal Binary qui doublerait la taille des libs whisper.cpp embarquées.
- Signature obligatoire : Apple Developer ID + Notarization via `xcrun notarytool` pour éviter les alertes Gatekeeper.
- Taille estimée : ~15–20 Mo par architecture.

**Windows :**
- Format : `.msi` via WiX (géré par tauri-bundler).
- Builds : x64 (prioritaire) et ARM64 (secondaire).
- Variantes : une build avec CUDA (pour GPU NVIDIA), une build CPU-only/Vulkan (universelle). L'installeur détecte et propose la bonne variante.
- Signature : Authenticode (certificat EV Code Signing recommandé pour éviter les alertes SmartScreen).
- Taille estimée : ~15–25 Mo.

**Linux :**
- Format principal : **AppImage** (portable, inclut toutes les libs sauf drivers GPU).
- Formats secondaires : `.deb`, `.rpm` pour installation système.
- Flatpak envisageable en phase post-v1.0 (gestion des portails xdg pour micro et raccourcis).
- Signature : GPG.
- Taille estimée : ~15–20 Mo.

### 10.2 CI/CD

GitHub Actions avec **runners natifs** par plateforme (pas de cross-compilation, pour éviter les problèmes de linking natif) :

- **macOS** : runner `macos-14` (ARM64). Builds arm64 et x86_64 séparées. Notarization automatisée.
- **Windows** : runner `windows-latest`. Builds x64 et ARM64. Signature Authenticode via `signtool`.
- **Linux** : runner `ubuntu-22.04` (compatibilité glibc maximale). Build x64 standard, ARM64 via cross-rs/QEMU.

**Compilation whisper.cpp et accélération matérielle :**

Chaque build compile whisper.cpp avec les flags d'accélération appropriés (`WHISPER_METAL=1`, `WHISPER_CUBLAS=1`, `WHISPER_VULKAN=1`).

**Point de vigilance : builds CUDA.** Les runners GitHub Actions hébergés n'ont pas de GPU NVIDIA, et les headers CUDA ne sont pas toujours disponibles. Stratégie retenue :
- **Option A (recommandée pour le MVP) :** télécharger les bibliothèques whisper.cpp pré-compilées avec CUDA depuis les releases officielles du projet whisper.cpp, et les lier au moment du build Tauri. Moins propre mais pragmatique et fiable.
- **Option B (cible v1.0) :** utiliser des self-hosted runners avec CUDA toolkit installé, ou dockeriser l'environnement de build Windows avec les headers CUDA.
- La build CPU-only/Vulkan reste la build par défaut et ne nécessite aucune dépendance GPU spéciale.

### 10.3 Auto-updates

Plugin `tauri-plugin-updater` avec manifest JSON sur CDN statique. Vérification déclenchée **manuellement** par l'utilisateur (cohérent avec le principe zéro connexion non sollicitée). Option dans les réglages pour activer un check périodique (opt-in, fréquence configurable). Chaque update est signée cryptographiquement.

### 10.4 Intégration système

| Fonctionnalité     | Plugin Tauri                   | Mécanisme natif                                           |
|--------------------|--------------------------------|-----------------------------------------------------------|
| Tray système       | `tauri-plugin-tray`            | NSStatusItem / Shell_NotifyIcon / libappindicator         |
| Autostart          | `tauri-plugin-autostart`       | LaunchAgents / Registry Run / XDG autostart               |
| Raccourcis globaux | `tauri-plugin-global-shortcut` | CGEvent / RegisterHotKey / XGrabKey + portail XDG Wayland |
| Notifications      | `tauri-plugin-notification`    | APIs natives par OS                                       |

---

## 11. Dépendances

### 11.1 Crates Rust (Cargo.toml)

| Crate                            | Version | Rôle                                                     |
|----------------------------------|---------|----------------------------------------------------------|
| `tauri`                          | 2.x     | Framework applicatif desktop                             |
| `whisper-rs`                     | 0.15+   | Bindings Rust pour whisper.cpp (inclut VAD natif Silero) |
| `cpal`                           | 0.15+   | Capture audio cross-platform                             |
| `tokio`                          | 1.x     | Runtime async                                            |
| `reqwest`                        | 0.12+   | HTTP client (téléchargements, API calls)                 |
| `serde` / `serde_json`           | 1.x     | Sérialisation                                            |
| `arboard`                        | 3.x     | Accès clipboard cross-platform                           |
| `enigo`                          | 0.2+    | Simulation de frappe clavier                             |
| `keyring`                        | 3.x     | Stockage sécurisé des clés API (keyring natif OS)        |
| `ringbuf`                        | 0.4+    | Ring buffer lock-free pour audio                         |
| `zeroize`                        | 1.x     | Zéroisation mémoire sécurisée                            |
| `sha2`                           | 0.10+   | Vérification d'intégrité des modèles                     |
| `tracing` / `tracing-subscriber` | 0.1.x   | Logging structuré local                                  |
| `dirs`                           | 5.x     | Chemins standard par OS                                  |
| `toml`                           | 0.8+    | Fichiers de configuration                                |

### 11.2 Packages Frontend (package.json)

| Package                     | Rôle                     |
|-----------------------------|--------------------------|
| `react` + `react-dom`       | Bibliothèque UI          |
| `@tauri-apps/api`           | Bridge IPC Tauri         |
| `tailwindcss`               | Framework CSS utilitaire |
| `vite`                      | Build tool et dev server |
| `zustand`                   | State management léger   |
| `lucide-react`              | Icônes                   |
| `i18next` + `react-i18next` | Internationalisation     |

---

## 12. Plan de test et validation

### 12.1 Stratégie de test

| Catégorie          | Périmètre                                       | Outils                          | Fréquence         |
|--------------------|-------------------------------------------------|---------------------------------|-------------------|
| Unitaire (Rust)    | Logique métier, parsers, pipeline audio, config | `cargo test`, `proptest`        | Chaque commit     |
| Intégration        | Transcription end-to-end, ModelManager, IPC     | `cargo test` + fixtures audio   | Chaque PR         |
| UI                 | Composants React, interactions                  | Vitest + Testing Library        | Chaque PR         |
| E2E cross-platform | Flux complet sur les 3 OS                       | WebdriverIO + Tauri driver      | Release candidate |
| Performance        | Latence transcription, mémoire, démarrage       | `criterion` + benchmarks custom | Hebdomadaire      |
| Sécurité / Privacy | Zéro connexion réseau en mode local             | Wireshark + tests d'intégration | Chaque release    |
| Hardware           | Metal, CUDA, Vulkan, CPU variants               | Matrice de machines physiques   | Release candidate |

### 12.2 Environnements physiques requis

La validation complète des composants audio et GPU nécessite des machines physiques (non fiables en VM) :
- Mac Apple Silicon (M1+) pour Metal
- PC Windows avec GPU NVIDIA récent pour CUDA
- PC Linux (Ubuntu 22.04+) avec GPU pour Vulkan
- Configurations CPU-only sur les trois OS
- Corpus audio de référence : plusieurs langues, conditions variées (bruit, accents, débits)

---

## 13. Roadmap MVP → v1.0

### Phase 1 — Fondations (Semaines 1–4)

**Objectif :** squelette fonctionnel sur macOS (plateforme de développement principale).

- Setup Tauri v2 + React + TypeScript + Vite
- **PrivacyGuard : HTTP client centralisé (singleton `reqwest::Client`)** — toute requête réseau transite par ce composant dès le jour 1. En mode local, le client bloque toute requête. Ceci évite que des appels `reqwest` isolés ne contournent le firewall applicatif.
- Intégration cpal : capture audio basique → buffer mémoire, avec machine à état `AudioManager`
- Intégration whisper-rs : transcription d'un fichier audio de test (CPU only)
- Pipeline complète : enregistrement → transcription → affichage texte dans l'UI
- Raccourci clavier global (toggle start/stop)
- Téléchargement automatique du modèle tiny GGUF au premier lancement
- **Livrable :** prototype fonctionnel sur macOS

### Phase 2 — MVP utilisable (Semaines 5–8)

**Objectif :** application utilisable au quotidien, premiers builds cross-platform.

- ModelManager : téléchargement, intégrité SHA-256, stockage, sélection de modèle dans l'UI
- HardwareDetector : détection automatique + recommandation de modèle
- Insertion texte dans l'application active (arboard + enigo, avec délai de synchronisation)
- Silero VAD natif (GGML via whisper-rs, modèle ~864 Ko téléchargé avec le premier modèle Whisper)
- UI tray système + fenêtre de réglages
- **Signature binaire et notarization macOS** — intégrer dès cette phase pour dérisquer. La notarization Apple et le SmartScreen Windows prennent systématiquement plus de temps que prévu.
- Première compilation et tests manuels sur Windows et Linux
- **Livrable :** MVP interne signé, builds cross-platform fonctionnels

### Phase 3 — Cross-platform et robustesse (Semaines 9–12)

**Objectif :** parité fonctionnelle sur les trois OS, privacy opérationnelle.

- Résolution des spécificités par OS (permissions micro, insertion texte, Wayland)
- Accélération GPU : Metal (macOS), CUDA (Windows/Linux), Vulkan (fallback)
- **Signature et notarization validées sur les 3 OS** (Authenticode Windows, GPG Linux)
- Module PrivacyGuard complet : audit trail local, indicateurs visuels, zéroisation mémoire
- Mode API opt-in (OpenAI Whisper API en premier)
- Gestionnaire de modèles complet dans l'UI (liste, téléchargement, suppression)
- Tests E2E automatisés sur les trois OS
- **Livrable :** bêta privée cross-platform, signée et notarisée

### Phase 4 — Polissage et v1.0 (Semaines 13–16)

**Objectif :** application prête pour distribution publique.

- Mode streaming (transcription quasi-temps-réel)
- Onboarding first-run (assistant de configuration guidé)
- Auto-updater fonctionnel (tauri-plugin-updater)
- Connecteurs API additionnels (Deepgram, AssemblyAI, endpoint custom)
- Packaging final : validation des installeurs sur machines propres (fresh install)
- Documentation utilisateur et page de téléchargement
- Optimisation mémoire et CPU finales (profiling sur les trois OS)
- **Livrable :** v1.0 publique

### Jalons récapitulatifs

| Jalon     | Semaine | Livrable                                                       | Plateformes              |
|-----------|---------|----------------------------------------------------------------|--------------------------|
| Prototype | S4      | Pipeline audio → transcription → texte, HTTP singleton         | macOS                    |
| MVP       | S8      | App utilisable, signée et notarisée                            | macOS + builds Win/Linux |
| Bêta      | S12     | Parité fonctionnelle, privacy, GPU, API opt-in, signature 3 OS | macOS + Windows + Linux  |
| v1.0      | S16     | Distribution publique, auto-updater, streaming                 | macOS + Windows + Linux  |

---

## 14. Défis techniques majeurs identifiés

Cinq risques techniques ont été identifiés et doivent être adressés en priorité :

**1. Accélération GPU sous Windows et Linux.** La diversité des configurations GPU (NVIDIA CUDA, AMD ROCm, Intel, Vulkan générique) et de drivers rend la détection et l'activation de l'accélération matérielle complexe. Les runners CI/CD hébergés n'ont pas de GPU, compliquant les builds CUDA. **Mitigation :** variantes de builds (CUDA vs CPU-only/Vulkan), détection automatique avec fallback gracieux, bibliothèques pré-compilées pour la CI.

**2. Wayland et raccourcis globaux sous Linux.** L'architecture de sécurité de Wayland interdit la capture globale de touches. Le portail XDG GlobalShortcuts n'est pas encore universellement supporté. **Mitigation :** fallback via commande CLI et socket IPC local, documentation utilisateur pour la configuration au niveau du compositeur.

**3. Injection de texte fiable cross-platform.** La simulation de `Ctrl+V` / `Cmd+V` via `enigo` peut échouer dans certaines applications. Le presse-papier X11 fonctionne par négociation asynchrone, source fréquente de bugs de timing. **Mitigation :** délai de synchronisation pré-paste (50–100ms), mécanisme de retry, fallback caractère par caractère, écriture dans les deux sélections X11.

**4. Résilience audio (hot-plug de périphériques).** La déconnexion/reconnexion de périphériques audio en cours de session (AirPods, casques USB, changement de sortie par défaut) peut interrompre silencieusement le stream cpal. **Mitigation :** machine à état `AudioManager` avec auto-recovery et notification UI.

**5. Notarization et signature binaire.** La notarization macOS (Gatekeeper) et la signature Authenticode Windows (SmartScreen) prennent systématiquement plus de temps que prévu lors de la première mise en place. **Mitigation :** intégrée dès la Phase 2 de la roadmap pour dérisquer avant la bêta.

---

*Fin du document — OpenSay Architecture Technique v1.1*
