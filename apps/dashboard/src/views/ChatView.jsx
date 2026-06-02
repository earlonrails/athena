import React, { useState, useEffect, useRef } from 'react';
import { Terminal, CheckCircle, Code, Loader2 } from 'lucide-react';

export default function ChatView() {
  const [messages, setMessages] = useState([
    { role: 'agent', content: 'Hello! I am Athena. How can I help you today?' }
  ]);
  const [input, setInput] = useState('');
  const [ws, setWs] = useState(null);
  const [config, setConfig] = useState(null);
  const [toolActivity, setToolActivity] = useState([]);
  const [isTyping, setIsTyping] = useState(false);
  const endRef = useRef(null);
  const toolsEndRef = useRef(null);

  const fetchConfig = () => {
    fetch('/api/config')
      .then(r => r.json())
      .then(setConfig)
      .catch(console.error);
  };

  useEffect(() => {
    fetchConfig();
    
    const socket = new WebSocket(`ws://${window.location.host}/api/chat`);
    
    socket.onopen = () => {
      console.log('Connected to chat server');
    };

    socket.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        if (data.TokenDelta) {
          setMessages(prev => {
            const newMessages = [...prev];
            const lastMsg = newMessages[newMessages.length - 1];
            if (lastMsg && lastMsg.role === 'agent' && lastMsg.isStreaming) {
              lastMsg.content += data.TokenDelta;
            } else {
              newMessages.push({ role: 'agent', content: data.TokenDelta, isStreaming: true });
            }
            return newMessages;
          });
        } else if (data.ToolCallStart) {
          setIsTyping(true);
          setToolActivity(prev => [...prev, { ...data.ToolCallStart, status: 'running' }]);
        } else if (data.ToolCallComplete) {
          setToolActivity(prev => prev.map(t => 
            t.id === data.ToolCallComplete.id ? { ...t, status: 'complete', result: data.ToolCallComplete.result } : t
          ));
        } else if (data.FinalResponse) {
          setIsTyping(false);
          setMessages(prev => {
            const newMessages = [...prev];
            const lastMsg = newMessages[newMessages.length - 1];
            if (lastMsg && lastMsg.role === 'agent' && lastMsg.isStreaming) {
              lastMsg.isStreaming = false;
              lastMsg.content = data.FinalResponse;
            } else {
               // Fallback if no tokens arrived
               newMessages.push({ role: 'agent', content: data.FinalResponse });
            }
            return newMessages;
          });
        } else if (data.Error) {
          setIsTyping(false);
          setMessages(prev => [...prev, { role: 'agent', content: `**Error:** ${data.Error}`, isError: true }]);
        }
      } catch (e) {
        // Fallback for raw text
        setMessages(prev => [...prev, { role: 'agent', content: event.data }]);
      }
    };

    socket.onclose = () => {
      console.log('Disconnected from chat server');
    };

    setWs(socket);

    return () => socket.close();
  }, []);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages]);

  useEffect(() => {
    toolsEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [toolActivity]);

  const sendMessage = (e) => {
    e.preventDefault();
    if (!input.trim() || !ws) return;

    const text = input.trim();
    setMessages(prev => [...prev, { role: 'user', content: text }]);
    setIsTyping(true);
    ws.send(text);
    setInput('');
  };

  const changeModel = async (newProvider) => {
    if (!config) return;
    const clone = { ...config };
    if (!clone.model) clone.model = {};
    clone.model.provider = newProvider;
    
    if (newProvider === 'openai') clone.model.default = 'gpt-4o';
    else if (newProvider === 'anthropic') clone.model.default = 'claude-3-5-sonnet-20240620';
    else if (newProvider === 'mistral') clone.model.default = 'mistral-large-latest';
    else clone.model.default = '';

    try {
      await fetch('/api/config', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(clone)
      });
      setConfig(clone);
    } catch (e) {
      console.error(e);
    }
  };

  return (
    <div className="view-container" style={{ height: '100%', display: 'flex', gap: '20px', flexDirection: 'row' }}>
      
      {/* Main Chat Column */}
      <div style={{ flex: 2, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '16px' }}>
          <h1 className="page-title" style={{ marginBottom: 0 }}>Agent Chat</h1>
          {config && (
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <span style={{ fontSize: '0.85rem', color: 'var(--text-muted)' }}>Active Model:</span>
              <select 
                value={config.model?.provider || ''} 
                onChange={e => changeModel(e.target.value)}
                className="glass-select"
              >
                <option value="openai">OpenAI ({config.model?.provider === 'openai' ? config.model?.default : 'gpt-4o'})</option>
                <option value="anthropic">Anthropic ({config.model?.provider === 'anthropic' ? config.model?.default : 'claude-3-5-sonnet'})</option>
                <option value="gemini">Gemini</option>
                <option value="mistral">Mistral ({config.model?.provider === 'mistral' ? config.model?.default : 'mistral-large'})</option>
                <option value="openrouter">OpenRouter</option>
                <option value="deepseek">DeepSeek</option>
                <option value="groq">Groq</option>
                <option value="xai">xAI</option>
              </select>
            </div>
          )}
        </div>

        <div className="chat-window" style={{ flex: 1 }}>
          <div className="chat-messages">
            {messages.map((msg, i) => (
              <div key={i} className={`chat-bubble ${msg.role} ${msg.isError ? 'error' : ''}`}>
                <div className="msg-content">{msg.content}</div>
                {msg.isStreaming && <span className="streaming-cursor"></span>}
              </div>
            ))}
            {isTyping && messages[messages.length-1]?.role === 'user' && (
              <div className="chat-bubble agent typing-indicator">
                <Loader2 size={18} className="spin" /> Thinking...
              </div>
            )}
            <div ref={endRef} />
          </div>
          <form className="chat-input-area glass-panel" onSubmit={sendMessage}>
            <input 
              type="text" 
              placeholder="Type your message..." 
              value={input}
              onChange={(e) => setInput(e.target.value)}
              className="glass-input"
            />
            <button type="submit" className="glass-btn primary">Send</button>
          </form>
        </div>
      </div>

      {/* Tool Activity Sidebar */}
      <div className="tool-sidebar glass-panel" style={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
        <h3 style={{ margin: '0 0 16px 0', borderBottom: '1px solid var(--border-dim)', paddingBottom: '12px', display: 'flex', alignItems: 'center', gap: '8px' }}>
          <Terminal size={18} /> Tool Activity
        </h3>
        <div className="tool-activity-list" style={{ overflowY: 'auto', flex: 1, paddingRight: '8px' }}>
          {toolActivity.length === 0 ? (
            <div style={{ color: 'var(--text-muted)', textAlign: 'center', marginTop: '40px', fontSize: '0.9rem' }}>
              No tools executed yet.
            </div>
          ) : (
            toolActivity.map((tool, idx) => (
              <div key={idx} className={`tool-item ${tool.status}`}>
                <div className="tool-header">
                  {tool.status === 'running' ? <Loader2 size={14} className="spin" /> : <CheckCircle size={14} className="success-icon" />}
                  <span className="tool-name">{tool.name}</span>
                </div>
                <div className="tool-args">
                  <Code size={12} /> {tool.arguments}
                </div>
                {tool.result && (
                  <div className="tool-result">
                    {tool.result.length > 150 ? tool.result.substring(0, 150) + '...' : tool.result}
                  </div>
                )}
              </div>
            ))
          )}
          <div ref={toolsEndRef} />
        </div>
      </div>
    </div>
  );
}
