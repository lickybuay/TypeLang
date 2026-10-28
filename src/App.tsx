import Popup from "./popup/Popup";
import Pill from "./pill/Pill";
import SnippetsPicker from "./snippets/SnippetsPicker";
import SettingsPage from "./settings/SettingsPage";
import "./App.css";

function App() {
  if (window.location.hash.startsWith("#/popup")) return <Popup />;
  if (window.location.hash.startsWith("#/pill")) return <Pill />;
  if (window.location.hash.startsWith("#/snippets")) return <SnippetsPicker />;
  return <SettingsPage />;
}

export default App;
