import { BrowserRouter, Routes, Route } from "react-router-dom";
import { Nav } from "./components/Nav";
import { Footer } from "./components/Footer";
import { Home } from "./pages/Home";
import { Download } from "./pages/Download";
import { Security } from "./pages/Security";

export function App() {
  return (
    <BrowserRouter>
      <Nav />
      <main>
        <Routes>
          <Route path="/" element={<Home />} />
          <Route path="/download" element={<Download />} />
          <Route path="/security" element={<Security />} />
        </Routes>
      </main>
      <Footer />
    </BrowserRouter>
  );
}
