#ifndef VAULTSERVICE_H
#define VAULTSERVICE_H

#include "../../core/ports/icryptomanager.h"
#include "../../core/ports/igitmanager.h"
#include "../../core/ports/istoragemanager.h"

#include <string>
#include <system_error>

class VaultService
{
public:
	VaultService(ICryptoManger& cryptoManager, IGitManager& gitManager, IStorageManager& storageManager);
	~VaultService();

	int Open(const std::string& vault_path, const uint8_t* master_pswd, size_t master_pswd_len);

private:
	IGitManager& _gitManager;
	ICryptoManger& _cryptoManager;
	IStorageManager& _storageManager;
};
#endif
