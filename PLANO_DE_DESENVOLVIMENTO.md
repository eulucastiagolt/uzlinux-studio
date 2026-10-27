# Engineering & Development Plan: Uzlinux Studio

> **Architecture and Port Engineering Document in Rust**  
> **Target Devices:** Ulanzi Stream Controller D200/D200H (`e9-02C53A015U3671380`), Elgato Stream Deck compatible devices, and Virtual Decks.  
> **Interop Reference:** OpenDeck (`me.amankhanna.opendeck`, v2.14.2) and Stream Deck Plugin Ecosystem.

---

## 1. Executive Summary

The goal of **Uzlinux Studio** is to deliver a native, high-performance, and modular control deck application in Rust for modern Linux systems. Focused primarily on **Ulanzi** hardware integration, it maintains full interoperability with:
1. **Elgato Stream Deck and OpenAction plugins** (`.streamDeckPlugin` / `.sdPlugin`).
2. **HTML/JS Property Inspectors** running in isolated WebViews.
3. **OpenDeck profiles and settings** (`settings.json`, `applications.json`, `profiles/<device>/<profile>.json`).
4. **Hardware Devices:**
   - Deep native integration for **Ulanzi Studio Deck D200 / D200H** (`DeviceNamespace: e9`), eliminating third-party bridge overhead.
   - Virtual Decks and generic controllers.

---

## 2. Technical Documentation

For the complete, up-to-date documentation suite, please refer to:
- 🛠️ [Plugin Development Guide](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs/PLUGIN_DEVELOPMENT_GUIDE.md)
- 🤖 [AI-Assisted Plugin Creation Guide](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs/AI_CREATION_GUIDE.md)
- 📐 [Architecture & Engineering Plan](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs/DEVELOPMENT_PLAN.md)
- 🔬 [Technical Reverse Engineering & Compatibility Report](file:///home/lucastiago/Documentos/Projetos/Rust/UzlinuxStudioDeck/docs/REVERSE_ENGINEERING_REPORT.md)
