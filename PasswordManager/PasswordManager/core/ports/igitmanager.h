#ifndef IGITMANAGER_H
#define IGITMANAGER_H

class IGitManager
{
public:
    virtual ~IGitManager() = default;

	virtual void Push() = 0;
	virtual void Pull() = 0;
};


#endif // IGITMANAGER_H
