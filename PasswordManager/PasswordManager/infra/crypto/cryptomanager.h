#include "../../core/ports/icryptomanager.h"

#ifndef CRYPTOMANAGER_H
#define CRYPTOMANAGER_H

class CryptoManager : ICryptoManger
{
public:
	void Encrypt() override;
	void Decrypt(std::string& master_pswd) override;
};

#endif // CRYPTOMANAGER_H