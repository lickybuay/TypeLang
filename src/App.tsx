import Popup from "./popup/Popup";
import SettingsPage from "./settings/SettingsPage";
import "./App.css";

function App() {
  const isPopup = window.location.hash.startsWith("#/popup");
  return isPopup ? <Popup /> : <SettingsPage />;
}

export default App;
