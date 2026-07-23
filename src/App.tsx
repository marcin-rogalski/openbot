import { invoke } from "@tauri-apps/api/core"

function App() {
  const hideToTray = (): void => {
    void invoke("hide_main_window")
  }

  return (
    <main>
      <h1>openbot</h1>
      <p>Replace this with your app. The tray icon and window plumbing already work.</p>
      <button type="button" onClick={hideToTray}>
        Hide to tray
      </button>
    </main>
  )
}

export default App
