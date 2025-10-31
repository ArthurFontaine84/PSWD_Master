#ifndef VAULT_MANAGER_H
#define VAULT_MANAGER_H

#include <iostream>
#include <fstream>
#include "Constants.h"
#include "Windows.h"

class VaultManager
{
public:
	VaultManager(std::string MasterPswd);
	~VaultManager();

private:
	bool FileExists(std::string filePath);

	// Champs :
	std::string _masterPassword;
};

#endif // !VAULT_MANAGER_H
