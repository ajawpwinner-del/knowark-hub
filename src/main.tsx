import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import { UiProvider } from './components/design';
import { HubProvider } from './store';
import './styles/app.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <UiProvider>
      <HubProvider>
        <App />
      </HubProvider>
    </UiProvider>
  </React.StrictMode>
);
