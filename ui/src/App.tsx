import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface PrivacyConfig {
  local_only: boolean;
  allowed_domains: string[];
}

interface LoggingConfig {
  level: string;
  file_logging: boolean;
  max_files: number;
}

interface UiConfig {
  show_tray: boolean;
  start_minimized: boolean;
  theme: string;
}

interface TranscriptionConfig {
  model: string;
  language: string;
  vad_enabled: boolean;
}

interface AppConfig {
  privacy: PrivacyConfig;
  logging: LoggingConfig;
  ui: UiConfig;
  transcription: TranscriptionConfig;
}

interface AppPaths {
  data_dir: string;
  logs_dir: string;
  config_path: string;
}

function App() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [paths, setPaths] = useState<AppPaths | null>(null);
  const [networkBlocked, setNetworkBlocked] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function loadData() {
      try {
        const [configData, pathsData, blocked] = await Promise.all([
          invoke<AppConfig>("get_config"),
          invoke<AppPaths>("get_paths"),
          invoke<boolean>("is_network_blocked"),
        ]);
        setConfig(configData);
        setPaths(pathsData);
        setNetworkBlocked(blocked);
      } catch (e) {
        setError(e as string);
      }
    }
    loadData();
  }, []);

  if (error) {
    return (
      <div className="min-h-screen bg-gray-900 text-white p-8">
        <div className="max-w-2xl mx-auto">
          <h1 className="text-2xl font-bold text-red-500 mb-4">Error</h1>
          <p className="text-red-300">{error}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gray-900 text-white p-8">
      <div className="max-w-2xl mx-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-8">
          <h1 className="text-3xl font-bold">OpenSay</h1>
          <PrivacyBadge networkBlocked={networkBlocked} />
        </div>

        {/* Privacy Status */}
        <section className="mb-8 p-6 bg-gray-800 rounded-lg">
          <h2 className="text-xl font-semibold mb-4">Privacy Status</h2>
          <div className="flex items-center gap-3">
            {networkBlocked ? (
              <>
                <div className="w-4 h-4 rounded-full bg-green-500 animate-pulse" />
                <span className="text-green-400 font-medium">
                  Local Mode (Network Blocked)
                </span>
              </>
            ) : (
              <>
                <div className="w-4 h-4 rounded-full bg-orange-500 animate-pulse" />
                <span className="text-orange-400 font-medium">
                  Network Mode (API Enabled)
                </span>
              </>
            )}
          </div>
          <p className="mt-2 text-gray-400 text-sm">
            {networkBlocked
              ? "All network requests are blocked. Your audio never leaves this device."
              : "Network access is enabled for cloud transcription APIs."}
          </p>
        </section>

        {/* Paths */}
        {paths && (
          <section className="mb-8 p-6 bg-gray-800 rounded-lg">
            <h2 className="text-xl font-semibold mb-4">Application Paths</h2>
            <div className="space-y-2 text-sm font-mono">
              <div>
                <span className="text-gray-400">Config: </span>
                <span className="text-gray-200">{paths.config_path}</span>
              </div>
              <div>
                <span className="text-gray-400">Data: </span>
                <span className="text-gray-200">{paths.data_dir}</span>
              </div>
              <div>
                <span className="text-gray-400">Logs: </span>
                <span className="text-gray-200">{paths.logs_dir}</span>
              </div>
            </div>
          </section>
        )}

        {/* Configuration */}
        {config && (
          <section className="mb-8 p-6 bg-gray-800 rounded-lg">
            <h2 className="text-xl font-semibold mb-4">Configuration</h2>
            <pre className="bg-gray-900 p-4 rounded overflow-x-auto text-sm">
              {JSON.stringify(config, null, 2)}
            </pre>
          </section>
        )}

        {/* Footer */}
        <footer className="text-center text-gray-500 text-sm">
          <p>OpenSay v0.1.0 - Privacy-first voice transcription</p>
        </footer>
      </div>
    </div>
  );
}

function PrivacyBadge({ networkBlocked }: { networkBlocked: boolean }) {
  return (
    <div
      className={`px-3 py-1 rounded-full text-sm font-medium ${
        networkBlocked
          ? "bg-green-900 text-green-300 border border-green-700"
          : "bg-orange-900 text-orange-300 border border-orange-700"
      }`}
    >
      {networkBlocked ? "Local" : "Cloud"}
    </div>
  );
}

export default App;
