#ifndef GITMANAGER_H
#define GITMANAGER_H

#include "../../core/ports/igitmanager.h"

class GitManager : IGitManager
{
	void Push() override;
	void Pull() override;
};

#endif // GITMANAGER_H