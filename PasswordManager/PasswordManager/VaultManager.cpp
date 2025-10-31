#include "VaultManager.h"  

VaultManager::VaultManager(std::string MasterPswd)  
{  
  // Vérifie si le fichier json.enc existe  
  FileExists(Constants::JSON_ENC);  
}  

VaultManager::~VaultManager()  
{  
}  

bool VaultManager::FileExists(std::string filePath)  
{  
  std::string path = filePath;  
  std::ifstream file(path);
  char buffer[MAX_PATH];
  GetCurrentDirectoryA(MAX_PATH, buffer);
  std::cout << "Répertoire courant : " << buffer << std::endl;

  //std::cout << "Répertoire courant : "
  //    << std::filesystem::current_path() << std::endl;
  return true;
}
