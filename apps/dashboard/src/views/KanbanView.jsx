import React, { useState, useEffect } from 'react';
import { Plus, GripVertical, Trash2, User } from 'lucide-react';

export default function KanbanView() {
  const [cards, setCards] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);

  const columns = [
    { id: 'col-todo', title: 'TODO' },
    { id: 'col-in-progress', title: 'IN PROGRESS' },
    { id: 'col-done', title: 'DONE' }
  ];

  const fetchKanban = async () => {
    try {
      setLoading(true);
      const res = await fetch('/api/kanban');
      const data = await res.json();
      if (data.cards) setCards(data.cards);
    } catch (err) {
      setError(err.message);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchKanban();
  }, []);

  const handleCreate = async () => {
    const title = prompt("Enter task title:");
    if (!title) return;
    
    await fetch('/api/kanban', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'create', title })
    });
    fetchKanban();
  };

  const handleMove = async (taskId, columnId) => {
    await fetch('/api/kanban', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'move', task_id: taskId, column_id: columnId })
    });
    fetchKanban();
  };

  const handleDelete = async (taskId) => {
    if (!confirm("Are you sure you want to delete this task?")) return;
    await fetch('/api/kanban', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'delete', task_id: taskId })
    });
    fetchKanban();
  };

  const handleAssign = async (taskId) => {
    const assignee = prompt("Enter assignee name:");
    if (!assignee) return;
    await fetch('/api/kanban', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ action: 'assign', task_id: taskId, assignee })
    });
    fetchKanban();
  };

  if (loading) return <div className="p-8 text-center glass-panel m-6">Loading Kanban...</div>;

  return (
    <div className="view-container flex flex-col h-full" style={{ padding: '20px' }}>
      <div className="flex justify-between items-center mb-6">
        <h1 className="text-2xl font-bold bg-clip-text text-transparent bg-gradient-to-r from-cyan-400 to-indigo-400">
          Agent Kanban Board
        </h1>
        <button onClick={handleCreate} className="glass-btn flex items-center gap-2" style={{ padding: '8px 16px' }}>
          <Plus size={18} /> New Task
        </button>
      </div>

      {error && <div className="error-box text-red-400 mb-4">{error}</div>}

      <div className="flex gap-6 overflow-x-auto pb-4 h-full">
        {columns.map(col => {
          const colCards = cards.filter(c => c.column_id === col.id);
          
          return (
            <div key={col.id} className="glass-panel flex-1 min-w-[300px] flex flex-col" style={{ background: 'rgba(15, 23, 42, 0.4)' }}>
              <div className="flex justify-between items-center mb-4 border-b border-gray-700 pb-2">
                <h3 className="font-bold text-cyan-300">{col.title} <span className="text-gray-500 text-sm ml-2">({colCards.length})</span></h3>
              </div>
              
              <div className="flex-1 overflow-y-auto pr-2 space-y-3">
                {colCards.map(card => (
                  <div key={card.id} className="bg-black/30 p-4 rounded-xl border border-gray-700 hover:border-cyan-500 transition-colors group">
                    <div className="flex justify-between items-start mb-2">
                      <h4 className="font-semibold text-gray-200">{card.title}</h4>
                      <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                        <button onClick={() => handleDelete(card.id)} className="text-red-400 hover:text-red-300"><Trash2 size={16} /></button>
                      </div>
                    </div>
                    
                    <div className="text-xs text-gray-500 mb-4 font-mono">#{card.id.substring(0,8)}</div>
                    
                    <div className="flex justify-between items-center mt-4">
                      <div 
                        className="flex items-center gap-1 text-sm bg-gray-800/50 px-2 py-1 rounded cursor-pointer hover:bg-gray-700/50 transition-colors text-indigo-300"
                        onClick={() => handleAssign(card.id)}
                      >
                        <User size={14} /> {card.assignee || 'Unassigned'}
                      </div>
                      
                      <select 
                        className="glass-select text-xs" 
                        value={card.column_id}
                        onChange={(e) => handleMove(card.id, e.target.value)}
                      >
                        {columns.map(c => (
                          <option key={c.id} value={c.id} className="bg-slate-900 text-white">{c.title}</option>
                        ))}
                      </select>
                    </div>
                  </div>
                ))}
                {colCards.length === 0 && (
                  <div className="text-center text-gray-600 italic mt-8 border-2 border-dashed border-gray-700/50 rounded-xl p-4">Drop cards here</div>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
