/// <reference types="vite/client" />

interface Window {
  /** Set by debug builds started with KONNYAKU_DUMP; see src-tauri/src/dump.rs. */
  __KONNYAKU_DUMP__?: boolean;
}
