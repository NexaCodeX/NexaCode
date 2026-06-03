/* eslint-disable react-hooks/set-state-in-effect */
import { useState, useEffect, useCallback } from 'react';
import { useLLM } from '../hooks/useLLM';
import { LucideIcon } from './LucideIcon';

interface SettingsProps {
  isOpen: boolean;
  onClose: () => void;
}

export function Settings({ isOpen, onClose }: SettingsProps) {
  const [providers, setProviders] = useState<string[]>([]);
  const [activeProvider, setActiveProviderState] = useState<string | null>(null);
  const [showAddProvider, setShowAddProvider] = useState(false);
  const [editingProvider, setEditingProvider] = useState<string | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null);
  const [newProvider, setNewProvider] = useState({
    name: '',
    type: 'openai' as 'openai' | 'anthropic' | 'openai_compatible',
    apiKey: '',
    baseUrl: '',
    models: [] as string[],
  });

  const { addProvider, setActiveProvider, listProviders, getActiveProvider, removeProvider, getProviderConfig, updateProvider, listModels, error } = useLLM();

  const loadProviders = useCallback(async () => {
    const providerList = await listProviders();
    setProviders(providerList);
    const active = await getActiveProvider();
    setActiveProviderState(active);
  }, [listProviders, getActiveProvider]);

  useEffect(() => {
    if (isOpen) {
      loadProviders();
    }
  }, [isOpen, loadProviders]);

  const handleAddProvider = async () => {
    if (!newProvider.name || !newProvider.apiKey) {
      alert('Please fill in name and API key');
      return;
    }

    const success = await addProvider(
      newProvider.name,
      newProvider.type,
      newProvider.apiKey,
      newProvider.models,
      newProvider.baseUrl || undefined
    );

    if (success) {
      await loadProviders();
      setShowAddProvider(false);
      setNewProvider({
        name: '',
        type: 'openai',
        apiKey: '',
        baseUrl: '',
        models: [],
      });
    }
  };

  const handleSetActive = async (name: string) => {
    const success = await setActiveProvider(name);
    if (success) {
      await loadProviders();
    }
  };

  const handleRemoveProvider = (name: string) => {
    setDeleteConfirm(name);
  };

  const confirmDelete = async () => {
    if (!deleteConfirm) return;
    
    try {
      console.log('Calling removeProvider for:', deleteConfirm);
      await removeProvider(deleteConfirm);
      console.log('removeProvider succeeded');
      await loadProviders();
      console.log('Providers reloaded');
      setDeleteConfirm(null);
    } catch (err) {
      console.error('Failed to remove provider:', err);
      alert(`Failed to remove provider: ${err}`);
    }
  };

  const handleEditProvider = async (name: string) => {
    const config = await getProviderConfig(name);
    if (config) {
      setNewProvider({
        name: name,
        type: config.provider_type as 'openai' | 'anthropic' | 'openai_compatible',
        apiKey: config.api_key,
        baseUrl: config.base_url || '',
        models: config.models || [],
      });
      setEditingProvider(name);
      setShowAddProvider(false);
    }
  };

  const handleUpdateProvider = async () => {
    if (!editingProvider || !newProvider.apiKey || !newProvider.name) {
      alert('Please fill in name and API key');
      return;
    }

    // If name changed, delete old and create new
    if (newProvider.name !== editingProvider) {
      await removeProvider(editingProvider);
      const success = await addProvider(
        newProvider.name,
        newProvider.type,
        newProvider.apiKey,
        newProvider.models,
        newProvider.baseUrl || undefined
      );
      if (success) {
        await loadProviders();
        setEditingProvider(null);
        setNewProvider({
          name: '',
          type: 'openai',
          apiKey: '',
          baseUrl: '',
          models: [],
        });
      }
    } else {
      // Just update the config
      const success = await updateProvider(
        editingProvider,
        newProvider.type,
        newProvider.apiKey,
        newProvider.models,
        newProvider.baseUrl || undefined
      );

      if (success) {
        await loadProviders();
        setEditingProvider(null);
        setNewProvider({
          name: '',
          type: 'openai',
          apiKey: '',
          baseUrl: '',
          models: [],
        });
      }
    }
  };

  const handleLoadModels = async () => {
    const currentActive = await getActiveProvider();
    const targetProvider = editingProvider || newProvider.name;
    
    if (!targetProvider) return;
    
    if (currentActive !== targetProvider) {
      await setActiveProvider(targetProvider);
    }
    
    const models = await listModels();
    const modelIds = models.map(m => m.id);
    
    setNewProvider(prev => ({
      ...prev,
      models: [...new Set([...prev.models, ...modelIds])],
    }));
    
    if (currentActive && currentActive !== targetProvider) {
      await setActiveProvider(currentActive);
    }
  };

  const handleAddModel = () => {
    setNewProvider(prev => ({
      ...prev,
      models: [...prev.models, ''],
    }));
  };

  const handleRemoveModel = (model: string) => {
    setNewProvider(prev => ({
      ...prev,
      models: prev.models.filter(m => m !== model),
    }));
  };

  if (!isOpen) return null;

  const renderProviderForm = (isEdit: boolean) => (
    <div className="add-provider-form">
      <h4>{isEdit ? `Edit Provider` : 'Add New Provider'}</h4>
      
      <div className="form-group">
        <label>Name *</label>
        <input
          type="text"
          value={newProvider.name}
          onChange={(e) => setNewProvider({ ...newProvider, name: e.target.value })}
          placeholder="e.g., my-openai"
        />
        {isEdit && newProvider.name !== editingProvider && (
          <small className="form-hint" style={{ color: 'var(--accent-warning)' }}>
            ⚠️ Changing name will create a new provider
          </small>
        )}
      </div>

      <div className="form-group">
        <label>Type *</label>
        <select
          value={newProvider.type}
          onChange={(e) => setNewProvider({ ...newProvider, type: e.target.value as 'openai' | 'anthropic' | 'openai_compatible' })}
        >
          <option value="openai">OpenAI</option>
          <option value="anthropic">Anthropic (Claude)</option>
          <option value="openai_compatible">OpenAI Compatible (Ollama, etc.)</option>
        </select>
      </div>

      <div className="form-group">
        <label>API Key *</label>
        <input
          type="password"
          value={newProvider.apiKey}
          onChange={(e) => setNewProvider({ ...newProvider, apiKey: e.target.value })}
          placeholder="sk-..."
        />
      </div>

      <div className="form-group">
        <label>Base URL {newProvider.type === 'openai_compatible' ? '*' : '(Optional)'}</label>
        <input
          type="text"
          value={newProvider.baseUrl}
          onChange={(e) => setNewProvider({ ...newProvider, baseUrl: e.target.value })}
          placeholder={
            newProvider.type === 'openai' 
              ? 'https://api.openai.com/v1 (or your proxy)' 
              : newProvider.type === 'anthropic'
              ? 'https://api.anthropic.com/v1 (or your proxy)'
              : 'http://localhost:11434/v1'
          }
        />
        <small className="form-hint">
          {newProvider.type === 'openai' && 'Custom base URL for OpenAI API proxy or alternative endpoint'}
          {newProvider.type === 'anthropic' && 'Custom base URL for Anthropic API proxy or alternative endpoint'}
          {newProvider.type === 'openai_compatible' && 'Required for local models: Ollama, vLLM, LM Studio, etc.'}
        </small>
      </div>

      <div className="form-group">
        <div className="label-row">
          <label>Models</label>
          <button onClick={handleLoadModels} className="load-link">
            load from API
          </button>
        </div>
        <div className="models-list-editor">
          {newProvider.models.map((model, index) => (
            <div key={index} className="model-row">
              <input
                type="text"
                value={model}
                onChange={(e) => {
                  const newModels = [...newProvider.models];
                  newModels[index] = e.target.value;
                  setNewProvider(prev => ({ ...prev, models: newModels }));
                }}
                placeholder="model-name"
              />
              <button 
                onClick={() => handleRemoveModel(model)} 
                className="remove-model-btn"
                title="Remove model"
              >
                <LucideIcon name="trash-2" size={16} color="var(--accent-coral)" />
              </button>
            </div>
          ))}
          <button onClick={handleAddModel} className="add-model-row-btn">
            <LucideIcon name="plus" size={16} color="var(--text-secondary)" />
            <span>Add Model</span>
          </button>
        </div>
      </div>

      {error && <div className="error-message">{error}</div>}

      <div className="form-actions">
        <button onClick={() => {
          if (isEdit) {
            setEditingProvider(null);
          } else {
            setShowAddProvider(false);
          }
          setNewProvider({
            name: '',
            type: 'openai',
            apiKey: '',
            baseUrl: '',
            models: [],
          });
        }}>Cancel</button>
        <button onClick={isEdit ? handleUpdateProvider : handleAddProvider} className="primary">
          {isEdit ? 'Update Provider' : 'Add Provider'}
        </button>
      </div>
    </div>
  );

  return (
    <div className="settings-overlay" onClick={onClose}>
      <div className="settings-modal" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h2>Settings</h2>
          <button className="close-btn" onClick={onClose}>
            <LucideIcon name="x" size={20} color="var(--text-secondary)" />
          </button>
        </div>

        <div className="settings-content">
          <div className="settings-section">
            <h3>LLM Providers</h3>
            
            {providers.length === 0 ? (
              <p className="no-providers">No providers configured. Add one to get started.</p>
            ) : (
              <div className="providers-list">
                {providers.map((provider) => (
                  <div key={provider} className={`provider-item ${activeProvider === provider ? 'active' : ''}`}>
                    <div className="provider-info">
                      <span className="provider-name">{provider}</span>
                      {activeProvider === provider && (
                        <span className="active-badge">Active</span>
                      )}
                    </div>
                    <div className="provider-actions">
                      {activeProvider !== provider && (
                        <button onClick={() => handleSetActive(provider)}>Set Active</button>
                      )}
                      <button onClick={() => handleEditProvider(provider)}>Edit</button>
                      <button 
                        onClick={(e) => {
                          e.stopPropagation();
                          handleRemoveProvider(provider);
                        }} 
                        className="remove-btn"
                        title="Remove provider"
                      >
                        <LucideIcon name="trash-2" size={16} color="var(--accent-coral)" />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {showAddProvider ? (
              renderProviderForm(false)
            ) : editingProvider ? (
              renderProviderForm(true)
            ) : (
              <button className="add-provider-btn" onClick={() => setShowAddProvider(true)}>
                <LucideIcon name="plus" size={16} color="var(--text-secondary)" />
                <span>Add Provider</span>
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Delete Confirmation Dialog */}
      {deleteConfirm && (
        <div className="confirm-overlay" onClick={() => setDeleteConfirm(null)}>
          <div className="confirm-dialog" onClick={(e) => e.stopPropagation()}>
            <h3>Delete Provider</h3>
            <p>Are you sure you want to delete "{deleteConfirm}"?</p>
            <div className="confirm-actions">
              <button onClick={() => setDeleteConfirm(null)}>Cancel</button>
              <button onClick={confirmDelete} className="danger">Delete</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
