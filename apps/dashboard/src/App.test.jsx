import { render, screen } from '@testing-library/react';
import App from './App';

// Mock matchMedia
window.matchMedia = window.matchMedia || function() {
  return {
    matches: false,
    addListener: function() {},
    removeListener: function() {}
  };
};

window.HTMLElement.prototype.scrollIntoView = function() {};

// Mock WebSocket
class MockWebSocket {
  constructor(url) {
    this.url = url;
    this.readyState = 0; // CONNECTING
    
    setTimeout(() => {
      this.readyState = 1; // OPEN
      if (this.onopen) this.onopen();
    }, 0);
  }
  
  send(data) {}
  close() {}
}

// Mock fetch
global.fetch = function() {
  return Promise.resolve({
    json: () => Promise.resolve({}),
    ok: true
  });
};

global.WebSocket = MockWebSocket;

describe('App Dashboard', () => {
  it('renders the dashboard title', async () => {
    render(<App />);
    const titleElements = await screen.findAllByText(/ATHENA/i);
    expect(titleElements.length).toBeGreaterThan(0);
  });
});
