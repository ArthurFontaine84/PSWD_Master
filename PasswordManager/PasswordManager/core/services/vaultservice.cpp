#include "vaultservice.h"

VaultService::VaultService(ICryptoManger& cryptoManager, IGitManager& gitManager, IStorageManager& storageManager)
	: _gitManager(gitManager), _cryptoManager(cryptoManager), _storageManager(storageManager)
{
}

VaultService::~VaultService() {}

int VaultService::Open(const std::string& vault_path, const uint8_t* master_pswd, size_t master_pswd_len)
{
	
	_storageManager.Load(vault_path);

	return 0;
}
