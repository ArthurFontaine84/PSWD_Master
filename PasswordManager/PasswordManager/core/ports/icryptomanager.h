#ifndef ICRYPTOMANAGER_H
#define ICRYPTOMANAGER_H

#include <string>

class ICryptoManger  
{  
public:  
   virtual ~ICryptoManger() = default; 

   virtual void Encrypt();
   virtual void Decrypt(std::string& master_pswd);
};

#endif // ICRYPTOMANAGER_H