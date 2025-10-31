class GitManager
{
public:
	GitManager();
	~GitManager();

	bool IsOnline();
	bool Pull();
	bool Push();
};
