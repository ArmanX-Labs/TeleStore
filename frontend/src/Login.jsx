import React, { useState } from 'react';
import './Login.css';

const Login = ({ onLoginSuccess }) => {
  const [step, setStep] = useState(1); // 1: phone, 2: code, 3: password
  const [phone, setPhone] = useState('');
  const [code, setCode] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);

  const handleSendCode = async (e) => {
    e.preventDefault();
    setLoading(true);
    setError('');
    try {
      const res = await fetch('/api/auth/send_code', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ phone })
      });
      if (!res.ok) throw new Error(await res.text());
      const data = await res.json();
      if (data.token) {
        localStorage.setItem('mycloud_token', data.token);
        onLoginSuccess();
      } else {
        setStep(2);
      }
    } catch (err) {
      setError(err.message);
    }
    setLoading(false);
  };

  const handleSignIn = async (e) => {
    e.preventDefault();
    setLoading(true);
    setError('');
    try {
      const res = await fetch('/api/auth/sign_in', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ phone, code })
      });
      if (!res.ok) throw new Error(await res.text());
      const tokenOrNull = await res.json();
      if (tokenOrNull === null) {
        setStep(3);
      } else {
        localStorage.setItem('mycloud_token', tokenOrNull);
        onLoginSuccess();
      }
    } catch (err) {
      setError(err.message);
    }
    setLoading(false);
  };

  const handlePassword = async (e) => {
    e.preventDefault();
    setLoading(true);
    setError('');
    try {
      const res = await fetch('/api/auth/check_password', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ phone, password })
      });
      if (!res.ok) throw new Error(await res.text());
      const token = await res.json();
      localStorage.setItem('mycloud_token', token);
      onLoginSuccess();
    } catch (err) {
      setError(err.message);
    }
    setLoading(false);
  };

  return (
    <div className="login-container">
      <div className="login-card">
        <h2>MyCloud Login</h2>
        <p>Connect your Telegram account to sync your files.</p>
        
        {error && <div className="error-msg">{error}</div>}

        {step === 1 && (
          <form onSubmit={handleSendCode}>
            <div className="input-group">
              <label>Phone Number</label>
              <input type="text" placeholder="+1234567890" value={phone} onChange={e => setPhone(e.target.value)} required />
            </div>
            <button type="submit" disabled={loading} className="btn-primary">
              {loading ? 'Sending...' : 'Send Code'}
            </button>
          </form>
        )}

        {step === 2 && (
          <form onSubmit={handleSignIn}>
            <div className="input-group">
              <label>Telegram Code</label>
              <input type="text" placeholder="12345" value={code} onChange={e => setCode(e.target.value)} required />
            </div>
            <button type="submit" disabled={loading} className="btn-primary">
              {loading ? 'Signing in...' : 'Sign In'}
            </button>
          </form>
        )}

        {step === 3 && (
          <form onSubmit={handlePassword}>
            <div className="input-group">
              <label>2FA Password</label>
              <input type="password" placeholder="Password" value={password} onChange={e => setPassword(e.target.value)} required />
            </div>
            <button type="submit" disabled={loading} className="btn-primary">
              {loading ? 'Verifying...' : 'Verify Password'}
            </button>
          </form>
        )}
      </div>
    </div>
  );
};

export default Login;
