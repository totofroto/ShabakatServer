// Dynamically use the current domain. 
// In development, Vite proxies /api to localhost:7779.
// In production, we assume the frontend is served by the same server or a proxy on the same origin.
export const API_BASE_URL = typeof window !== 'undefined' ? window.location.origin : '';
