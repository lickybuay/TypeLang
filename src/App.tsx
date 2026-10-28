import Popup from "./popup/Popup";
import SnippetsPicker from "./snippets/SnippetsPicker";
import SettingsPage from "./settings/SettingsPage";
import "./App.css";

function App() {
  if (window.location.hash.startsWith("#/popup")) return <Popup />;
  if (window.location.hash.startsWith("#/snippets")) return <SnippetsPicker />;
  return <SettingsPage />;
}

export default App;
