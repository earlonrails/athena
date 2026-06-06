import React, { useState } from 'react';
import { BrowserRouter as Router, Routes, Route, Link, useLocation } from 'react-router-dom';
import { MessageSquare, Settings, Puzzle, Box, Wrench, Menu, X, Cpu } from 'lucide-react';

import ChatView from './views/ChatView.jsx';
import ConfigView from './views/ConfigView.jsx';
import ModelView from './views/ModelView.jsx';
import SkillsView from './views/SkillsView.jsx';
import MCPView from './views/MCPView.jsx';
import PluginsView from './views/PluginsView.jsx';
import ToolsView from './views/ToolsView.jsx';
import KanbanView from './views/KanbanView.jsx';

function Sidebar({ isOpen, toggleSidebar }) {
  const location = useLocation();
  const navLinks = [
    { path: '/', icon: <MessageSquare size={20} />, label: 'Chat' },
    { path: '/kanban', icon: <Box size={20} />, label: 'Kanban' },
    { path: '/models', icon: <Cpu size={20} />, label: 'Models' },
    { path: '/config', icon: <Settings size={20} />, label: 'Configuration' },
    { path: '/skills', icon: <Puzzle size={20} />, label: 'Skills' },
    { path: '/mcp', icon: <Box size={20} />, label: 'MCP Servers' },
    { path: '/tools', icon: <Wrench size={20} />, label: 'Tools' },
    { path: '/plugins', icon: <Box size={20} />, label: 'Plugins' },
  ];

  return (
    <>
      {isOpen && <div className="mobile-overlay" onClick={toggleSidebar}></div>}
      
      <aside className={`sidebar ${isOpen ? 'open' : ''}`}>
        <div className="sidebar-header">
          <span className="owl-icon">🦉</span>
          <span className="logo-text">ATHENA</span>
          <button className="close-btn" onClick={toggleSidebar}>
            <X size={24} />
          </button>
        </div>
        
        <nav className="nav-menu">
          {navLinks.map((link) => {
            const isActive = location.pathname === link.path;
            return (
              <Link
                key={link.path}
                to={link.path}
                onClick={() => { if (window.innerWidth < 768) toggleSidebar(); }}
                className={`nav-item ${isActive ? 'active' : ''}`}
              >
                {link.icon}
                <span>{link.label}</span>
              </Link>
            );
          })}
        </nav>
      </aside>
    </>
  );
}

export default function App() {
  const [sidebarOpen, setSidebarOpen] = useState(false);

  return (
    <Router>
      <div className="app-container">
        <Sidebar isOpen={sidebarOpen} toggleSidebar={() => setSidebarOpen(!sidebarOpen)} />
        
        <div className="main-content-wrapper">
          <header className="mobile-header">
            <button className="menu-btn" onClick={() => setSidebarOpen(true)}>
              <Menu size={24} />
            </button>
            <span className="logo-text">ATHENA</span>
          </header>
          
          <main className="main-content">
            <div className="glow-orb orb-1"></div>
            <div className="glow-orb orb-2"></div>
            
            <Routes>
              <Route path="/" element={<ChatView />} />
              <Route path="/kanban" element={<KanbanView />} />
              <Route path="/models" element={<ModelView />} />
              <Route path="/config" element={<ConfigView />} />
              <Route path="/skills" element={<SkillsView />} />
              <Route path="/mcp" element={<MCPView />} />
              <Route path="/tools" element={<ToolsView />} />
              <Route path="/plugins" element={<PluginsView />} />
            </Routes>
          </main>
        </div>
      </div>
    </Router>
  );
}
