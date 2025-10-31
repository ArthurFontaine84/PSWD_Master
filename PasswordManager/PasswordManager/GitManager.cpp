#include "GitManager.h"
#include <iostream>

GitManager::GitManager()
{
}

GitManager::~GitManager()
{
}

bool GitManager::IsOnline()
{
    int result = system("git ls-remote origin >nul 2>&1");
    if (result != 0) {
        std::cout << "Pas de connexion au depot distant, utilisation du dernier enregistrement en local.\n";
        return false;
    }

    std::cout << "Connexion git reussie !\n";
	return true;
}

bool GitManager::Pull()
{
	int result = system("git pull origin main");
	if (result != 0) {
		std::cout << "Echec de la synchronisation.\n";
		return false;
	}

	std::cout << "Synchronisation reussie.\n";
}

bool GitManager::Push()
{
	return false;
}
