import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import { AppClientProvider } from "./presentation/AppClientContext";
import { TablesPage } from "./presentation/pages/TablesPage";
import { TableGridPage } from "./presentation/pages/TableGridPage";
import { RowDetailPage } from "./presentation/pages/RowDetailPage";
import "./App.css";

function App() {
  return (
    <AppClientProvider>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<Navigate to="/tables" replace />} />
          <Route path="/tables" element={<TablesPage />} />
          <Route path="/tables/:id" element={<TableGridPage />} />
          <Route path="/rows/:id" element={<RowDetailPage />} />
        </Routes>
      </BrowserRouter>
    </AppClientProvider>
  );
}

export default App;
