#include <iostream>  

#include "GitManager.h"
#include "VaultManager.h"

int main() {
	GitManager git;

	if (git.IsOnline()) { // Vérifie si le dépôt est accessible
		git.Pull();
	}
	else {
		std::cout << "Le dépôt Git n'est pas accessible en ligne." << std::endl;
	}

	// Demande master Passwdord
	std::string MasterPassword;
	std::cout << "Entrez le mot de passe maitre : ";
	std::cin >> MasterPassword;

	// Init VaultManager avec le MasterPassword
	VaultManager vault(MasterPassword);


}