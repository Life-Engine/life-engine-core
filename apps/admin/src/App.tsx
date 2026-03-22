import { Route, Routes } from "react-router-dom";
import Layout from "./components/Layout";
import DashboardPage from "./pages/DashboardPage";
import ConfigPage from "./pages/ConfigPage";
import PluginsPage from "./pages/PluginsPage";
import SystemPage from "./pages/SystemPage";

export default function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<DashboardPage />} />
        <Route path="config" element={<ConfigPage />} />
        <Route path="plugins" element={<PluginsPage />} />
        <Route path="system" element={<SystemPage />} />
      </Route>
    </Routes>
  );
}
